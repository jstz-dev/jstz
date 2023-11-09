use anyhow::{anyhow, Result};
use jstz_proto::operation::{Content, DeployContract, Operation, SignedOperation};

use crate::{
    config::Config,
    jstz::JstzClient,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub async fn exec(
    self_address: Option<String>,
    contract_code: Option<String>,
    balance: u64,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = JstzClient::new(cfg);

    // resolve contract code
    let contract_code = contract_code
        .map(from_file_or_id)
        .or_else(piped_input)
        .ok_or(anyhow!("No function code supplied"))?;

    let account = cfg.accounts.account_or_current_mut(self_address)?;

    let nonce = jstz_client
        .get_nonce(account.address.clone().to_base58().as_str())
        .await?;

    // Create operation
    let op = Operation {
        source: account.address.clone(),
        nonce,
        content: Content::DeployContract(DeployContract {
            contract_code,
            contract_credit: balance,
        }),
    };

    let signed_op = SignedOperation::new(
        account.public_key.clone(),
        account.secret_key.sign(op.hash())?,
        op,
    );

    let hash = signed_op.hash();

    println!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // Send message to jstz
    OctezClient::send_rollup_external_message(
        cfg,
        "bootstrap2",
        bincode::serialize(&signed_op)?,
    )?;

    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    cfg.save()?;

    Ok(())
}
