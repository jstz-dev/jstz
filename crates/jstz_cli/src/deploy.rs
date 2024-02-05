use jstz_proto::{
    operation::{Content, DeployContract, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};

use crate::{
    account::account::OwnedAccount,
    config::Config,
    error::{bail, bail_user_error, user_error, Result},
    utils::read_file_or_input_or_piped,
};

pub async fn exec(
    self_address: Option<String>,
    contract_code: Option<String>,
    balance: u64,
    name: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let account = cfg.accounts.account_or_current(self_address)?;

    // Check if account already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) {
            bail_user_error!("A function with the alias '{}' already exists.", name);
        }
    }

    let jstz_client = cfg.jstz_client()?;

    let nonce = jstz_client
        .get_nonce(account.address().clone().to_base58().as_str())
        .await?;

    // Check if account is Owned
    let OwnedAccount {
        address,
        secret_key,
        public_key,
        ..
    } = account.as_owned()?.clone();

    let contract_code = read_file_or_input_or_piped(contract_code)?
        .ok_or(user_error!("No function code supplied. Please provide a filename or pipe the file contents into stdin."))?;

    // Create operation TODO nonce
    let op = Operation {
        source: address,
        nonce,
        content: Content::DeployContract(DeployContract {
            contract_code,
            contract_credit: balance,
        }),
    };

    let signed_op = SignedOperation::new(public_key, secret_key.sign(op.hash())?, op);

    let hash = signed_op.hash();

    println!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // Send message to jstz
    cfg.jstz_client()?.post_operation(&signed_op).await?;

    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    // Create alias
    if let Some(name) = name {
        cfg.accounts.add_alias(
            name,
            match receipt.inner {
                Ok(ReceiptContent::DeployContract(deploy)) => {
                    deploy.contract_address.to_string()
                }
                _ => {
                    bail!("Expected a `DeployContract` receipt, but got something else.")
                }
            },
        )?;
    }

    cfg.save()?;

    Ok(())
}
