use anyhow::{anyhow, Result};
use jstz_proto::{
    operation::{Content, DeployContract, Operation, SignedOperation},
    receipt::Content as ReceiptContent,
};

use crate::{
    account::account::OwnedAccount,
    config::Config,
    sandbox::CLIENT_ADDRESS,
    utils::{from_file_or_id, piped_input},
};

pub async fn exec(
    self_address: Option<String>,
    contract_code: Option<String>,
    balance: u64,
    name: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    // Check if account already exists
    if let Some(name) = &name {
        if cfg.accounts.contains(name) {
            return Err(anyhow!("Account already exists"));
        }
    }

    let jstz_client = cfg.jstz_client()?;

    let account = cfg.accounts.account_or_current_mut(self_address)?;

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

    let contract_code = contract_code
        .map(from_file_or_id)
        .or_else(piped_input)
        .ok_or(anyhow!("No function code supplied"))?;

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
    cfg.octez_client()?
        .send_rollup_external_message(CLIENT_ADDRESS, bincode::serialize(&signed_op)?)?;

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
                _ => return Err(anyhow!("Content is not of type 'DeployContract'")),
            },
        )?;
    }

    cfg.save()?;

    Ok(())
}
