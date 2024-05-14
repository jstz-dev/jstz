use std::str::FromStr;

use crate::logs::{exec_trace, DEFAULT_LOG_LEVEL};
use anyhow::bail;
use http::{HeaderMap, Method, Uri};
use jstz_proto::context::account::Address;
use jstz_proto::{
    operation::{Content as OperationContent, Operation, RunFunction, SignedOperation},
    receipt::Content as ReceiptContent,
};
use log::{debug, info};
use spinners::{Spinner, Spinners};
use tokio::sync::mpsc;
use url::Url;

use crate::error::bail_user_error;
use crate::jstz::JstzClient;
use crate::{
    account,
    config::{Config, NetworkName},
    error::{anyhow, user_error, Result},
    term::styles,
    utils::{read_file_or_input_or_piped, AddressOrAlias},
};

// This was measured by running the benchmark.js,
// where the FA2 transfer function was called 1000 times.
pub const DEFAULT_GAS_LIMIT: u32 = 550000;

pub async fn exec(
    url: String,
    http_method: String,
    gas_limit: u32,
    json_data: Option<String>,
    network: Option<NetworkName>,
    trace: bool,
) -> Result<()> {
    // 1. Get the current user (checking if we are logged in)
    let mut cfg = Config::load()?;
    account::login_quick(&mut cfg)?;
    cfg.reload()?;

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

    let address_or_alias = AddressOrAlias::from_str(host)?;

    if address_or_alias.is_alias() {
        let address = address_or_alias.resolve(&cfg)?;

        info!("Resolved host '{}' to '{}'.", host, address);

        url_object
            .set_host(Some(&address.to_string()))
            .map_err(|_| anyhow!("Failed to set host"))?;
    }

    debug!("Resolved URL: {}", url_object.to_string());

    // 3. Construct the signed operation
    let nonce = jstz_client.get_nonce(&user.address).await?;

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
    println!(
        "Running function at {} ",
        styles::url(&url_object.to_string())
    );

    let mut spinner = if trace {
        None
    } else {
        Some(Spinner::new(Spinners::BoxBounce2, "".into()))
    };

    if trace {
        let address = address_or_alias.resolve(&cfg)?;
        spawn_trace(&address, &jstz_client).await?;
    }

    jstz_client.post_operation(&signed_op).await?;
    let receipt = jstz_client.wait_for_operation_receipt(&hash).await?;

    debug!("Receipt: {:?}", receipt);
    let (status_code, headers, body) = match receipt.inner {
        Ok(ReceiptContent::RunFunction(run_function)) => (
            run_function.status_code,
            run_function.headers,
            run_function.body,
        ),
        Ok(_) => bail!("Expected a `RunFunction` receipt, but got something else."),

        Err(err) => bail_user_error!("{err}"),
    };

    if let Some(spinner) = spinner.as_mut() {
        spinner.stop_with_symbol(&format!("Status code: {}", status_code));
    } else {
        info!("Status code: {}", status_code);
    }

    info!("Headers: {:?}", headers);
    if let Some(body) = body {
        info!("Body: {}", String::from_utf8_lossy(&body));
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
