use std::str::FromStr;

use anyhow::{anyhow, Result};
use http::{HeaderMap, Method, Uri};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::operation::{Content, Operation, RunContract, SignedOperation};

use crate::{
    account::account::Account,
    config::Config,
    jstz::JstzClient,
    octez::OctezClient,
    utils::{from_file_or_id, piped_input},
};

pub async fn exec(
    cfg: &mut Config,
    referrer: Option<String>,
    url: String,
    http_method: String,
    json_data: Option<String>,
) -> Result<()> {
    let account = cfg.accounts.account_or_current_mut(referrer)?;
    let (nonce,
        _alias,
        address,
        secret_key,
        public_key,
        _function_code) = match account {
        Account::Owned {
            nonce,
            alias,
            address,
            secret_key,
            public_key,
            function_code } => (nonce, address.clone(), alias.clone(), secret_key.clone(), public_key.clone(), function_code.clone()),
        _ => return Err(anyhow!("The account is an alias and cannot be used for the run of a smart function. Please use an owned account.")),
    };

    // Create operation TODO nonce
    let url: Uri = url
        .parse()
        .map_err(|_| anyhow!("Failed to parse URL: {}", url))?;

    let method =
        Method::from_str(&http_method).map_err(|_| anyhow!("Invalid HTTP method"))?;

    let body = json_data
        .map(from_file_or_id)
        .or_else(piped_input)
        .map(String::into_bytes);

    let op = Operation {
        source: PublicKeyHash::from_base58(address.as_str())?,
        nonce: nonce.clone(),
        content: Content::RunContract(RunContract {
            uri: url,
            method,
            headers: HeaderMap::default(),
            body,
        }),
    };

    nonce.increment();

    let signed_op =
        SignedOperation::new(public_key.clone(), secret_key.sign(op.hash())?, op);

    let hash = signed_op.hash();

    println!(
        "Signed operation: {}",
        serde_json::to_string_pretty(&serde_json::to_value(&signed_op)?)?
    );

    // Send message
    OctezClient::send_rollup_external_message(
        cfg,
        "bootstrap2",
        bincode::serialize(&signed_op)?,
    )?;

    let receipt = JstzClient::new(cfg)
        .wait_for_operation_receipt(&hash)
        .await?;

    println!("Receipt: {:?}", receipt);

    cfg.save()?;

    Ok(())
}
