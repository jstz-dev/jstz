use std::process::Command;
use serde_json::json;

pub fn run_contract(url: String, http_method: String, json_data: String, octez_client_path: String, octez_client_setup_args: Vec<String>) {
    let mut self_address = url.clone(); //todo
    let mut contract = url.clone(); //todo

    // Create JSON message
    let jmsg = json!({
        "Transaction": {
            "contract_address": {
                "Tz4": self_address
            },
            "contract_code": contract
        }
    });

    // Convert to external hex message
    let emsg = hex::encode(jmsg.to_string());
    let hex_string = format!("hex:[ \"{}\" ]", emsg);

    let args:Vec<&str> = octez_client_setup_args.iter()
        .map(|s| s.as_str())
        .chain(
            ["send",
             "smart",
             "rollup",
             "message",
             &hex_string,
             "from",
             "bootstrap2"].iter().cloned()
        )
        .collect();

    // Send message
    Command::new("client")
        .args(&args)
        .output()
        .expect("Failed to send message");
}
