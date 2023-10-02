use std::str::FromStr;

use crate::config::Config;
use crate::utils::handle_output;
use jstz_crypto::public_key_hash::PublicKeyHash;

use http::Method;
use jstz_kernel::inbox::{Transaction, ExternalMessage};

pub fn run_contract(
    referrer: String,
    url: String,
    http_method: String,
    json_data: Option<String>,
    cfg: &Config,
) {
    // Create transaction
    let tx = ExternalMessage::Transaction(Transaction {
        referrer: PublicKeyHash::from_base58(&referrer).unwrap(),
        url,
        method: Method::from_str(&http_method).unwrap(),
        body: json_data,
    });

    // Create JSON message
    let jmsg = serde_json::to_vec(&tx).unwrap();

    // println!("Message: {:?}", jmsg);

    // Convert to external hex message
    let emsg = hex::encode(jmsg);
    let hex_string = format!("hex:[ \"{}\" ]", emsg);

    // println!("Sending message: {}", hex_string);

    // Send message
    let output = cfg
        .octez_client_command()
        .args([
            "send",
            "smart",
            "rollup",
            "message",
            &hex_string,
            "from",
            "bootstrap2",
        ])
        .output();

    // println!("Output: {:?}", output);

    handle_output(&output);
}