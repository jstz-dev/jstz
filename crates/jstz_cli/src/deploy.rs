use boa_engine::JsError;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::{
    context::{new_account::NewAddress, new_account::ParsedCode},
    operation::{Content, DeployFunction, Operation, SignedOperation},
    receipt::{ReceiptContent, ReceiptResult},
};
use log::{debug, info};

use crate::{
    account,
    config::{Config, NetworkName, SmartFunction},
    error::{anyhow, bail, bail_user_error, user_error, Result},
    term::styles,
    utils::read_file_or_input_or_piped,
};

pub async fn exec(
    code_op: Option<String>,
    balance: u64,
    name: Option<String>,
    network: Option<NetworkName>,
) -> Result<()> {
    // maximum size of code until the DAL is implemented
    const MAX_CODE_LENGTH: usize = 3915;

    let mut cfg = Config::load().await?;
    // Load sandbox if the selected network is Dev and sandbox is not already loaded
    if cfg.network_name(&network)? == NetworkName::Dev && cfg.sandbox.is_none() {
        bail_user_error!(
            "No sandbox is currently running. Please run {}.",
            styles::command("jstz sandbox start")
        );
    }

    // Get the current user and check if we are logged in
    account::login_quick(&mut cfg).await?;
    cfg.reload().await?;
    let (user_name, user) = cfg.accounts.current_user().ok_or(anyhow!(
        "Failed to setup the account. Please run `{}`.",
        styles::command("jstz login")
    ))?;

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

    let code = read_file_or_input_or_piped(code_op)?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    if code.bytes().len() > MAX_CODE_LENGTH {
        bail_user_error!("The data availability layer is not yet available. Smart functions are currently restricted to {MAX_CODE_LENGTH} bytes");
    }

    debug!("Code: {}", code);

    let code: ParsedCode = code
        .try_into()
        .map_err(|err: JsError| user_error!("{err}"))?;
    // TODO: remove
    //  https://linear.app/tezos/issue/JSTZ-268/cli-use-publickeyhash-and-smartfunctionhash-in-user
    let source: Result<PublicKeyHash> = match user.address.clone() {
        NewAddress::User(address) => Ok(address),
        _ => bail!("address type mismatch - expected user address"),
    };
    let op = Operation {
        source: source?,
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

    let address = match receipt.result {
        ReceiptResult::Success(ReceiptContent::DeployFunction(deploy)) => deploy.address,
        ReceiptResult::Success(_) => {
            bail!("Expected a `DeployFunction` receipt, but got something else.")
        }
        ReceiptResult::Failed(err) => {
            bail_user_error!("Failed to deploy smart function with error {err:?}.")
        }
    };

    info!(
        "Smart function deployed by {} at address: {}",
        user_name, address
    );

    // Show message showing how to run the smart function
    // TODO: add --trace flag
    let network_flag = match network {
        Some(network) => format!(" --network {}", network),
        None => "".to_string(),
    };
    info!(
        "Run with `{}{}{}`",
        styles::command("jstz run "),
        styles::url(format!("tezos://{}/", address)),
        styles::command(network_flag)
    );

    if let Some(name) = name {
        cfg.accounts.insert(name, SmartFunction { address });
    }

    cfg.save()?;

    Ok(())
}
