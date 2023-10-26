use anyhow::{anyhow, Result};
use jstz_proto::{
    context::account::Nonce,
    operation::{Content, DeployContract, Operation, SignedOperation},
};

use crate::{
    config::Config,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub fn exec(
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

    let alias = cfg.accounts().choose_alias(self_address).clone();
    if alias.is_none() {
        println!("No account selected");
        return Ok(());
    }
    let account = cfg.accounts.get(&alias.unwrap()).unwrap();

    // Create operation TODO nonce
    let op = Operation {
        source: account.address.clone(),
        nonce: Nonce::new(0),
        content: Content::DeployContract(DeployContract {
            contract_code: contract_code,
            contract_credit: balance,
        }),
    };

    let signed_op = SignedOperation::new(
        account.public_key.clone(),
        account.secret_key.sign(op.hash())?,
        op,
    );

    let json_string = serde_json::to_string_pretty(
        &serde_json::to_value(&signed_op).expect("Failed to serialize to JSON value"),
    )
    .expect("Failed to serialize to JSON string");

    println!("{}", json_string);

    // Send message to jstz
    OctezClient::send_rollup_external_message(cfg, "bootstrap2", &json_string)
}
