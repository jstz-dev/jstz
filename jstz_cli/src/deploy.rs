use anyhow::{anyhow, Result};
use serde_json::json;

use crate::{
    config::Config,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub fn exec(
    self_address: String,
    contract_code: Option<String>,
    balance: u64,
    cfg: &Config,
) -> Result<()> {
    // resolve contract code
    let contract_code = contract_code
        .map(from_file_or_id)
        .or_else(piped_input)
        .ok_or(anyhow!("No function code supplied"))?;
    // Create JSON message
    let jmsg = json!({
        "DeployContract": {
            "originating_address": {
                "Tz4": self_address
            },
            "contract_code": contract_code,
            "initial_balance": balance
        }
    });

    // Send message to jstz
    OctezClient::send_rollup_external_message(cfg, "bootstrap2", &jmsg.to_string())
}
