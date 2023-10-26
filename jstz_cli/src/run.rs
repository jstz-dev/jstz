use std::str::FromStr;

use anyhow::Result;
use http::{HeaderMap, Method};
use jstz_proto::{
    context::account::Nonce,
    operation::{Content, Operation, RunContract, SignedOperation},
};

use crate::{
    config::Config,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub fn exec(
    cfg: &mut Config,
    referrer: Option<String>,
    url: String,
    http_method: String,
    json_data: Option<String>,
) -> Result<()> {
    let alias = cfg.accounts().choose_alias(referrer);
    if alias.is_none() {
        println!("No account selected");
        return Ok(());
    }
    let account = cfg.accounts.get(&alias.unwrap()).unwrap();

    // Create operation TODO nonce
    let op = Operation {
        source: account.address.clone(),
        nonce: Nonce::new(0),
        content: Content::RunContract(RunContract {
            uri: url.parse().expect("Failed to parse URI"),
            method: Method::from_str(&http_method).unwrap(),
            headers: HeaderMap::default(),
            body: json_data
                .map(from_file_or_id)
                .or_else(piped_input)
                .map(String::into_bytes),
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

    // Send message
    OctezClient::send_rollup_external_message(cfg, "bootstrap2", &json_string)
}
