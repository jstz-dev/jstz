use jstz_proto::{
    operation::{Content, DeployFunction, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};
use log::{debug, info};

use crate::{
    config::{Config, NetworkName, SmartFunction},
    error::{bail, bail_user_error, user_error, Result},
    utils::read_file_or_input_or_piped,
};

pub async fn exec(
    code: Option<String>,
    balance: u64,
    name: Option<String>,
    network: Option<NetworkName>,
) -> Result<()> {
    let mut cfg = Config::load()?;

    let (_, user) = cfg.accounts.current_user().ok_or(user_error!(
        "You are not logged in. Please run `jstz login`."
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

    let code = read_file_or_input_or_piped(code)?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    debug!("Code: {}", code);

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

    info!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // 3. Send operation to jstz-node
    jstz_client.post_operation(&signed_op).await?;
    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    debug!("Receipt: {:?}", receipt);

    let address = match receipt.inner {
        Ok(ReceiptContent::DeployFunction(deploy)) => deploy.address,
        _ => {
            bail!("Expected a `DeployFunction` receipt, but got something else.")
        }
    };

    println!("Smart function deployed at address: {}", address);

    // 4. Save smart function account (if named)
    if let Some(name) = name {
        cfg.accounts.insert(name, SmartFunction { address });
    }

    cfg.save()?;

    Ok(())
}
