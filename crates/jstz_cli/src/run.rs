use std::str::FromStr;

use anyhow::bail;
use http::{HeaderMap, Method, Uri};
use jstz_proto::context::account::{Address, Addressable};
use jstz_proto::executor::JSTZ_HOST;
use jstz_proto::{
    operation::{Content as OperationContent, Operation, RunFunction, SignedOperation},
    receipt::{ReceiptContent, ReceiptResult},
};
use log::{debug, info};
use serde_json::Value;
use tokio::sync::mpsc;
use url::Url;

use crate::{
    account,
    config::{Config, NetworkName},
    error::{anyhow, bail_user_error, user_error, Result},
    jstz::JstzClient,
    logs::{exec_trace, DEFAULT_LOG_LEVEL},
    term::styles,
    utils::{read_file_or_input_or_piped, AddressOrAlias},
};

// This was measured by running the benchmark.js,
// where the FA2 transfer function was called 1000 times.
pub const DEFAULT_GAS_LIMIT: u32 = 550000;

pub enum Host {
    AddressOrAlias(AddressOrAlias),
    Jstz,
}

impl Host {
    pub fn resolve(&self, config: &Config) -> Result<String> {
        match self {
            Host::AddressOrAlias(address_or_alias) => {
                Ok(address_or_alias.resolve(config)?.to_base58())
            }
            Host::Jstz => Ok(JSTZ_HOST.to_string()),
        }
    }
}

impl TryFrom<&str> for Host {
    type Error = crate::error::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            JSTZ_HOST => Ok(Host::Jstz),
            _ => {
                let address_or_alias = AddressOrAlias::from_str(value)?;
                Ok(Host::AddressOrAlias(address_or_alias))
            }
        }
    }
}

pub async fn exec(
    url: String,
    http_method: String,
    gas_limit: u32,
    json_data: Option<String>,
    network: Option<NetworkName>,
    trace: bool,
    include_response_headers: bool,
) -> Result<()> {
    // 1. Get the current user (checking if we are logged in)
    let mut cfg = Config::load().await?;
    account::login_quick(&mut cfg).await?;
    cfg.reload().await?;

    let (_, user) = cfg.accounts.current_user().ok_or(anyhow!(
        "Failed to setup the account. Please run `{}`.",
        styles::command("jstz login")
    ))?;

    let jstz_client = cfg.jstz_client(&network)?;

    // 2. Resolve the URL
    let mut url_object = Url::parse(&url)
        .map_err(|_| user_error!("Invalid URL {}.", styles::url(&url)))?;

    let host = url_object
        .host_str()
        .ok_or(user_error!("URL {} requires a host.", styles::url(&url)))?;

    let parsed_host = Host::try_from(host)?;
    let resolved_host = parsed_host.resolve(&cfg)?;

    if host != resolved_host.as_str() {
        debug!("Resolved host '{}' to '{}'.", host, resolved_host);

        url_object
            .set_host(Some(&resolved_host.to_string()))
            .map_err(|_| anyhow!("Failed to set host"))?;
    }

    debug!("Resolved URL: {}", url_object.to_string());

    // 3. Construct the signed operation
    let nonce = jstz_client
        .get_nonce(&Address::User(user.address.clone()))
        .await?;

    // SAFETY: `url` is a valid URI since URLs are a subset of  URIs and `url_object` is a valid URL.
    let url: Uri = url_object
        .to_string()
        .parse()
        .expect("`url_object` is an invalid URL.");

    let method = Method::from_str(&http_method)
        .map_err(|_| user_error!("Invalid HTTP method: {}", http_method))?;

    debug!("Method: {:?}", method);

    let body = read_file_or_input_or_piped(json_data)?.map(String::into_bytes);

    debug!("Body: {:?}", body);

    let op = Operation {
        source: user.address.clone(),
        nonce,
        content: OperationContent::RunFunction(RunFunction {
            uri: url,
            method,
            headers: HeaderMap::default(),
            body,
            gas_limit: gas_limit
                .try_into()
                .map_err(|_| anyhow!("Invalid gas limit."))?,
        }),
    };

    debug!("Operation: {:?}", op);

    let hash = op.hash();

    debug!("Operation hash: {}", hash.to_string());

    let signed_op =
        SignedOperation::new(user.public_key.clone(), user.secret_key.sign(&hash)?, op);

    debug!("Signed operation: {:?}", signed_op);

    // 4. Send message to jstz node
    debug!(
        "Running function at {} ",
        styles::url(&url_object.to_string())
    );

    if trace {
        if let Host::AddressOrAlias(address_or_alias) = parsed_host {
            let address = address_or_alias.resolve(&cfg)?;
            spawn_trace(&address, &jstz_client).await?;
        }
    }

    jstz_client.post_operation(&signed_op).await?;
    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    debug!("Receipt: {:?}", receipt);
    let (status_code, headers, body) = match receipt.result {
        ReceiptResult::Success(ReceiptContent::RunFunction(run_function)) => (
            run_function.status_code,
            run_function.headers,
            run_function.body,
        ),
        ReceiptResult::Success(_) => {
            bail!("Expected a `RunFunction` receipt, but got something else.")
        }

        ReceiptResult::Failed(err) => bail_user_error!("{err}"),
    };

    if include_response_headers {
        info!("{}", status_code);
        for (key, value) in headers.iter() {
            let header_value = value.to_str();
            if let Ok(hval) = header_value {
                info!("{}: {}", key, hval);
            } else {
                debug!(
                    "Failed to parse header\nkey: '{}'\nvalue: {:?} ",
                    key, value
                );
            }
        }
        info!("\n")
    }

    if let Some(body) = body {
        let json = serde_json::from_slice::<Value>(&body)
            .and_then(|s| serde_json::to_string_pretty(&s));
        if json.is_ok() {
            info!("{}", json.unwrap());
        } else {
            let body = String::from_utf8(body);
            if body.is_ok() {
                info!("{}", body.unwrap());
            } else {
                info!("{:?}", body);
            }
        }
    }

    cfg.save()?;

    Ok(())
}

async fn spawn_trace(address: &Address, jstz_client: &JstzClient) -> Result<()> {
    let event_source = jstz_client.logs_stream(address);
    // need to use mpsc instead of oneshot because of the loop
    let (tx, mut rx) = mpsc::channel::<()>(1);

    tokio::spawn(async move {
        let _ = exec_trace(event_source, DEFAULT_LOG_LEVEL, || async {
            let _ = tx.send(()).await;
        })
        .await;
    });

    match rx.recv().await {
        Some(_) => {
            info!(
                "Connected to trace smart function {:?}",
                address.to_base58()
            );
            Ok(())
        }
        None => bail!("Failed to start trace."),
    }
}
