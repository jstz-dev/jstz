use std::str::FromStr;

use anyhow::{anyhow, Result};
use http::{HeaderMap, Method, Uri};
use jstz_proto::operation::{Content, Operation, RunContract, SignedOperation};

use crate::{
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
    gas_limit: Option<usize>,
    json_data: Option<String>,
) -> Result<()> {
    let account = cfg.accounts.account_or_current_mut(referrer)?;

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
        source: account.address.clone(),
        nonce: account.nonce.clone(),
        content: Content::RunContract(RunContract {
            uri: url,
            method,
            headers: HeaderMap::default(),
            gas_limit,
            body,
            gas_limit
        }),
    };

    account.nonce.increment();

    let signed_op = SignedOperation::new(
        account.public_key.clone(),
        account.secret_key.sign(op.hash())?,
        op,
    );

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
