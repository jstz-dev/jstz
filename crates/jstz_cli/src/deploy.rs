use jstz_proto::{
    operation::{Content, DeployContract, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};

use crate::{
    config::{Config, Deployment},
    error::{bail, bail_user_error, user_error, Result},
    utils::read_file_or_input_or_piped,
};

pub async fn exec(
    code: Option<String>,
    balance: u64,
    name: Option<String>,
) -> Result<()> {
    let mut cfg = Config::load()?;

    let (_, user) = cfg.accounts.current_user().ok_or(user_error!(
        "You are not logged in. Please run `jstz login`."
    ))?;

    // 1. Check if deployment account already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) {
            bail_user_error!(
                "A user/deployment with the alias '{}' already exists.",
                name
            );
        }
    }

    // 2. Construct operation
    let jstz_client = cfg.jstz_client()?;

    let nonce = jstz_client.get_nonce(&user.address).await?;

    let code = read_file_or_input_or_piped(code)?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    let op = Operation {
        source: user.address.clone(),
        nonce,
        content: Content::DeployContract(DeployContract {
            contract_code: code,
            contract_credit: balance,
        }),
    };

    let hash = op.hash();

    let signed_op =
        SignedOperation::new(user.public_key.clone(), user.secret_key.sign(&hash)?, op);

    println!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // 3. Send operation to jstz-node
    jstz_client.post_operation(&signed_op).await?;
    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    let address = match receipt.inner {
        Ok(ReceiptContent::DeployContract(deploy)) => deploy.contract_address,
        _ => {
            bail!("Expected a `DeployContract` receipt, but got something else.")
        }
    };

    println!("Smart function deployed at address: {}", address);

    // 4. Save deployment (if named)
    if let Some(name) = name {
        cfg.accounts.insert(name, Deployment { address });
    }

    cfg.save()?;

    Ok(())
}
