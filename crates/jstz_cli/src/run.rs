use std::str::FromStr;

use anyhow::{anyhow, Result};
use http::{HeaderMap, Method, Uri};
use jstz_proto::operation::{Content, Operation, RunContract, SignedOperation};
use url::Url;

use crate::{
    account::account::OwnedAccount,
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
    gas_limit: u32,
    json_data: Option<String>,
) -> Result<()> {
    let jstz_client = JstzClient::new(cfg);

    // Resolve URL
    let mut url_object =
        Url::parse(&url).map_err(|e| anyhow!("Failed to parse URL: {}", e))?;
    if let Some(host) = url_object.host_str() {
        if !host.starts_with("tz4") {
            if cfg.accounts().contains(host) {
                url_object
                    .set_host(Some(
                        cfg.accounts().get(host)?.address().to_base58().as_str(),
                    ))
                    .map_err(|_| anyhow!("Failed to set host"))?;
            } else {
                return Err(anyhow!("No such account exists."));
            }
        }
    } else {
        return Err(anyhow!("URL requires a host"));
    }

    let resolved_url = url_object.to_string();
    println!("Resolved URL: {}", resolved_url);

    let account = cfg
        .accounts
        .account_or_current_mut(referrer)?
        .as_owned_mut()?;
    let OwnedAccount {
        address,
        secret_key,
        public_key,
        alias: _,
    } = account;

    let nonce = jstz_client
        .get_nonce(address.clone().to_base58().as_str())
        .await?;

    // Create operation TODO nonce
    let url: Uri = resolved_url
        .parse()
        .map_err(|_| anyhow!("Failed to parse URL: {}", resolved_url))?;

    let method =
        Method::from_str(&http_method).map_err(|_| anyhow!("Invalid HTTP method"))?;

    let body = json_data
        .map(from_file_or_id)
        .or_else(piped_input)
        .map(String::into_bytes);

    let op = Operation {
        source: address.clone(),
        nonce,
        content: Content::RunContract(RunContract {
            uri: url,
            method,
            headers: HeaderMap::default(),
            body,
            gas_limit: gas_limit.try_into().unwrap_or(usize::MAX),
        }),
    };

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

    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    cfg.save()?;

    Ok(())
}
