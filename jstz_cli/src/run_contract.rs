use serde_json::json;
use crate::config::Config;
use crate::utils::handle_output;

pub fn run_contract(url: String, contract_code: String, /*http_method: String, json_data: String,*/ cfg: &Config) {
    // Create JSON message
    let jmsg = json!({
        "Transaction": {
            "contract_address": {
                "Tz4": url
            },
            "contract_code": contract_code
        }
    });

    // Convert to external hex message
    let emsg = hex::encode(jmsg.to_string());
    let hex_string = format!("hex:[ \"{}\" ]", emsg);

    // Send message
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
