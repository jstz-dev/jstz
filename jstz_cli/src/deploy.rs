use anyhow::{anyhow, Result};
use jstz_proto::operation::{Content, DeployContract, Operation, SignedOperation};

use crate::{
    config::Config,
    node::JstzNode,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub async fn exec(
    self_address: Option<String>,
    contract_code: Option<String>,
    balance: u64,
    cfg: &mut Config,
) -> Result<()> {
    // resolve contract code
    let contract_code = contract_code
        .map(from_file_or_id)
        .or_else(piped_input)
        .ok_or(anyhow!("No function code supplied"))?;

    let account = cfg.accounts.account_or_current_mut(self_address)?;

    // Create operation TODO nonce
    let op = Operation {
        source: account.address.clone(),
        nonce: account.nonce.clone(),
        content: Content::DeployContract(DeployContract {
            contract_code: contract_code,
            contract_credit: balance,
        }),
    };

    account.nonce.increment();

    let signed_op = SignedOperation::new(
        account.public_key.clone(),
        account.secret_key.sign(op.hash())?,
        op,
    );

    let hash = signed_op.hash();

    println!("Signed operation: {:?}", signed_op);

    // Send message to jstz
    OctezClient::send_rollup_external_message(
        cfg,
        "bootstrap2",
        bincode::serialize(&signed_op)?,
    )?;

    let receipt = JstzNode::new().wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    cfg.save()?;

    Ok(())
}
