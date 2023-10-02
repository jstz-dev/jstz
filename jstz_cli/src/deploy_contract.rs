use serde_json::json;
use crate::config::Config;
use crate::utils::handle_output;

pub fn deploy_contract(self_address: String, contract_code: String, balance: u64, cfg: &Config) {
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

    // Convert JSON to string
    let jmsg_str = jmsg.to_string();

    // Convert string to hexadecimal
    let emsg = hex::encode(jmsg_str);
    let hex_string = format!("hex:[ \"{}\" ]", emsg);

    let output = cfg.octez_client_command()
        .args(
            [
                "send",
                "smart",
                "rollup",
                "message",
                &hex_string,
                "from",
                "bootstrap2"
            ]
        )
        .output();

    handle_output(&output);
}
