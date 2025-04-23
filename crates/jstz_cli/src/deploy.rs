use boa_engine::JsError;
use jstz_core::reveal_data::MAX_REVEAL_SIZE;
use jstz_proto::{
    context::account::ParsedCode,
    operation::{Content, DeployFunction, Operation, SignedOperation},
    receipt::{ReceiptContent, ReceiptResult},
};
use log::{debug, info};

use crate::{
    account,
    config::{Config, NetworkName, SmartFunction},
    error::{anyhow, bail, bail_user_error, user_error, Result},
    sandbox::{assert_sandbox_running, JSTZD_SERVER_BASE_URL},
    term::styles,
    utils::{read_file_or_input_or_piped, Tez},
};

pub async fn exec(
    code_op: Option<String>,
    balance: Option<Tez>,
    name: Option<String>,
    network: Option<NetworkName>,
    force: bool,
) -> Result<()> {
    let mut cfg = Config::load().await?;
    // Load sandbox if the selected network is Dev and sandbox is not already loaded
    if cfg.network_name(&network)? == NetworkName::Dev {
        assert_sandbox_running(JSTZD_SERVER_BASE_URL).await?;
    }

    // Get the current user and check if we are logged in
    account::login_quick(&mut cfg).await?;
    cfg.reload().await?;
    let (user_name, user) = cfg.accounts.current_user().ok_or(anyhow!(
        "Failed to setup the account. Please run `{}`.",
        styles::command("jstz login")
    ))?;

    // 1. Check if the name already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) && !force {
            bail_user_error!(
                "The name '{}' is already used by another smart function or a user account. Please choose another name or specify the `--force` flag to overwrite the name.",
                name
            );
        }
    }

    // 2. Construct operation
    let jstz_client = cfg.jstz_client(&network)?;

    let nonce = jstz_client.get_nonce(&user.address.clone().into()).await?;

    debug!("Nonce: {:?}", nonce);

    let code = read_file_or_input_or_piped(code_op)?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    if code.bytes().len() > MAX_REVEAL_SIZE {
        bail_user_error!(
            "Smart functions are currently restricted to {MAX_REVEAL_SIZE} bytes"
        );
    }

    debug!("Code: {}", code);

    let code: ParsedCode = code
        .try_into()
        .map_err(|err: JsError| user_error!("{err}"))?;

    let op = Operation {
        public_key: user.public_key.clone(),
        nonce,
        content: Content::DeployFunction(DeployFunction {
            function_code: code,
            account_credit: balance.map(|b| b.to_mutez()).unwrap_or(0),
        }),
    };

    debug!("Operation: {:?}", op);

    let hash = op.hash();

    debug!("Operation hash: {}", hash.to_string());

    let signed_op = SignedOperation::new(user.secret_key.sign(&hash)?, op);

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
        styles::url(format!("jstz://{}/", address)),
        styles::command(network_flag)
    );

    if let Some(name) = name {
        cfg.accounts.insert(name, SmartFunction { address });
    }

    cfg.save()?;

    Ok(())
}
