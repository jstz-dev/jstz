use boa_engine::JsError;
use dialoguer::Input;
use jstz_proto::{
    context::account::ParsedCode,
    operation::{Content, DeployFunction, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};
use log::{debug, info};

use crate::{
    account,
    config::{Config, NetworkName, SmartFunction},
    error::{bail, bail_user_error, user_error, Result},
    sandbox::daemon,
    term::styles,
    utils::{get_file_name_from_path, read_file_or_input_or_piped},
};

pub async fn exec(
    code_op: Option<String>,
    balance: u64,
    name: Option<String>,
    network: Option<NetworkName>,
) -> Result<()> {
    // maximum size of code until the DAL is implemented
    const MAX_CODE_LENGTH: usize = 3915;

    let mut cfg = Config::load()?;

    // Load sandbox if network is Dev or it is None and cfg network is dev, and sandbox is not already loaded
    if (network == Some(NetworkName::Dev)
        || (network.is_none() && cfg.networks.default_network == Some(NetworkName::Dev)))
        && cfg.sandbox.is_none()
    {
        daemon::main(true, false, &mut cfg).await?;
        info!(
            "Use `{}` to start from a clear sandbox state.\n",
            styles::command("jstz sandbox restart --detach")
        );
        cfg = Config::load()?;
    }

    // Get the current user and check if we are logged in
    let (user_name, user) = {
        if cfg.accounts.current_user().is_none() {
            let account_alias: String = Input::new()
                    .with_prompt("You are not logged in. Please type the account name that you want to log into or create as new")
                    .interact()?;

            account::login(account_alias)?;
            println!();

            cfg = Config::load()?;
        }
        cfg.accounts.current_user().ok_or(user_error!(
            "Failed to setup the account. Please try `jstz login`."
        ))?
    };

    // 1. Check if smart function account already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) {
            bail_user_error!(
                "A user/smart function with the alias '{}' already exists.",
                name
            );
        }
    }

    // 2. Construct operation
    let jstz_client = cfg.jstz_client(&network)?;

    let nonce = jstz_client.get_nonce(&user.address).await?;

    debug!("Nonce: {:?}", nonce);

    let code = read_file_or_input_or_piped(code_op.clone())?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    if code.bytes().len() > MAX_CODE_LENGTH {
        bail_user_error!("The data availability layer is not yet available. Smart functions are currently restricted to {MAX_CODE_LENGTH} bytes");
    }
    let code_path = code_op.clone().unwrap_or_default();

    debug!("Code: {}", code);

    let code: ParsedCode = code
        .try_into()
        .map_err(|err: JsError| user_error!("{err}"))?;
    let op = Operation {
        source: user.address.clone(),
        nonce,
        content: Content::DeployFunction(DeployFunction {
            function_code: code,
            account_credit: balance,
        }),
    };

    debug!("Operation: {:?}", op);

    let hash = op.hash();

    debug!("Operation hash: {}", hash.to_string());

    let signed_op =
        SignedOperation::new(user.public_key.clone(), user.secret_key.sign(&hash)?, op);

    debug!("Signed operation: {:?}", signed_op);

    // 3. Send operation to jstz-node
    jstz_client.post_operation(&signed_op).await?;
    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    debug!("Receipt: {:?}", receipt);

    let address = match receipt.inner {
        Ok(ReceiptContent::DeployFunction(deploy)) => deploy.address,
        Ok(_) => {
            bail!("Expected a `DeployFunction` receipt, but got something else.")
        }
        Err(err) => {
            bail_user_error!("Failed to deploy smart function with error {err:?}.")
        }
    };

    // Create name if not provided
    let sf_name = if let Some(name) = name {
        name
    } else {
        let sf_name = {
            let mut name = "sf".to_string();
            let mut original_name = "sf".to_string();

            if let Some(file_name) = get_file_name_from_path(code_path.as_str()) {
                name = file_name.clone();
                original_name = file_name;
            }

            let mut i = 2;
            while cfg.accounts.contains(&name) {
                name = format!("{}_{}", original_name, i);
                i += 1;
            }
            name
        };

        sf_name
    };

    info!(
        "Smart function deployed by {} at address: {}={}",
        user_name, sf_name, address
    );

    cfg.accounts
        .insert(sf_name.clone(), SmartFunction { address });

    // Show message showing how to run the smart function
    // TODO: add --trace flag
    let network_flag = match network {
        Some(network) => format!(" --network {}", network),
        None => "".to_string(),
    };
    info!(
        "Run with `{}{}{}`",
        styles::command("jstz run "),
        styles::url(format!("tezos://{}/<args>", sf_name)),
        styles::command(network_flag)
    );

    cfg.save()?;

    Ok(())
}
