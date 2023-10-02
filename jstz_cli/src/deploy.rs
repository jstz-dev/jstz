use anyhow::Result;
use serde_json::json;

use crate::{config::Config, octez::OctezClient};

pub fn exec(
    self_address: String,
    contract_code: String,
    balance: u64,
    cfg: &Config,
) -> Result<()> {
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
