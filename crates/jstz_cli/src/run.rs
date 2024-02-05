use std::str::FromStr;

use http::{HeaderMap, Method, Uri};
use jstz_proto::operation::{Content, Operation, RunContract, SignedOperation};
use url::Url;

use crate::{
    account::account::OwnedAccount,
    config::Config,
    error::{anyhow, bail_user_error, user_error, Result},
    term::styles,
    utils::read_file_or_input_or_piped,
};

pub async fn exec(
    cfg: &mut Config,
    referrer: Option<String>,
    url: String,
    http_method: String,
    gas_limit: u32,
    json_data: Option<String>,
) -> Result<()> {
    let jstz_client = cfg.jstz_client()?;

    // Resolve URL
    let mut url_object = Url::parse(&url)
        .map_err(|_| user_error!("Invalid URL {}.", styles::url(&url)))?;

    if let Some(host) = url_object.host_str() {
        if !host.starts_with("tz1") {
            println!("Resolving host '{}'...", host);

            if cfg.accounts().contains(host) {
                let address = cfg.accounts().get(host)?.address().to_base58();

                println!("Resolved host '{}' to '{}'.", host, address);

                url_object
                    .set_host(Some(&address))
                    .map_err(|_| anyhow!("Failed to set host"))?;
            } else {
                bail_user_error!(
                    "The function '{}' is not known. Please add it to your accounts using `jstz account add`.",
                    host
                )
            }
        }
    } else {
        bail_user_error!("URL {} requires a host.", styles::url(&url));
    }

    // Login
    let account = cfg.accounts.account_or_current(referrer)?.as_owned()?;

    let OwnedAccount {
        address,
        secret_key,
        public_key,
        alias: _,
    } = account;

    let nonce = jstz_client
        .get_nonce(address.clone().to_base58().as_str())
        .await?;

    // SAFETY: `url` is a valid URI since URLs are a subset of  URIs and `url_object` is a valid URL.
    let url: Uri = url_object
        .to_string()
        .parse()
        .expect("`url_object` is an invalid URL.");

    let method = Method::from_str(&http_method)
        .map_err(|_| user_error!("Invalid HTTP method: {}", http_method))?;

    let body = read_file_or_input_or_piped(json_data)?.map(String::into_bytes);

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
    cfg.jstz_client()?.post_operation(&signed_op).await?;

    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    println!("Receipt: {:?}", receipt);

    cfg.save()?;

    Ok(())
}
