use std::process::Command;
use serde_json::json;

pub fn run_contract(url: String, http_method: String, json_data: String) {
    let root_dir = "..";

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

    // Send message
    Command::new("client")
        .arg("send")
        .arg("smart")
        .arg("rollup")
        .arg("message")
        .arg(format!("hex:[ \"{}\" ]", emsg))
        .arg("from")
        .arg("bootstrap2")
        .output()
        .expect("Failed to send message");
}
