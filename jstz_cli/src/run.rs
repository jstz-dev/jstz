use std::str::FromStr;

use anyhow::Result;
use http::Method;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_kernel::inbox::{ExternalMessage, Transaction};

use crate::{config::Config, octez::OctezClient};

pub fn exec(
    cfg: &Config,
    referrer: String,
    url: String,
    http_method: String,
    json_data: Option<String>,
) -> Result<()> {
    // Create transaction
    let tx = ExternalMessage::Transaction(Transaction {
        referrer: PublicKeyHash::from_base58(&referrer).unwrap(),
        url,
        method: Method::from_str(&http_method).unwrap(),
        body: json_data,
    });

    // Create JSON message
    let jmsg = serde_json::to_string(&tx).unwrap();

    // Send message
    OctezClient::send_rollup_external_message(cfg, "bootstrap2", &jmsg)
}
