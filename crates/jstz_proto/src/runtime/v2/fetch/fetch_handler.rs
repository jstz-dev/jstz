use crate::runtime::v2::fetch::error::{FetchError, Result};

use deno_core::{ByteString, JsBuffer, OpState, ResourceId};
use deno_fetch_base::{FetchHandler, FetchResponse, FetchReturn};
use std::{cell::RefCell, rc::Rc};

use jstz_core::host::JsHostRuntime;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_runtime::ProtocolContext;
use url::Url;

use crate::context::account::{Account, Address, AddressKind, Addressable};
use crate::runtime::v2::fetch::resources::{FetchRequestResource, FetchResponseResource};

use super::host_script::HostScript;
use super::http::{Body, Response, SupportedScheme};

use std::num::NonZeroU64;
use std::str::FromStr;
pub struct ProtoFetchHandler;

impl FetchHandler for ProtoFetchHandler {
    type CreateHttpClientArgs = ();

    type FetchError = FetchError;

    type Options = ();

    fn fetch(
        state: &mut OpState,
        method: ByteString,
        url: String,
        headers: Vec<(ByteString, ByteString)>,
        _client_rid: Option<u32>,
        _has_body: bool,
        data: Option<JsBuffer>,
        _resource: Option<ResourceId>,
    ) -> Result<FetchReturn> {
        let body = data.map(Body::Buffer);
        fetch(state, method, url, headers, body)
    }

    async fn fetch_send(
        state: Rc<RefCell<OpState>>,
        rid: ResourceId,
    ) -> Result<FetchResponse> {
        fetch_send(state, rid).await
    }

    fn custom_client(
        _state: &mut OpState,
        _args: Self::CreateHttpClientArgs,
    ) -> Result<ResourceId> {
        Err(FetchError::NotSupported(
            "custom_client op is not supported",
        ))
    }
}

fn fetch(
    state: &mut OpState,
    method: ByteString,
    url: String,
    headers: Vec<(ByteString, ByteString)>,
    body: Option<Body>,
) -> Result<FetchReturn> {
    let url = Url::try_from(url.as_str())?;
    let protocol = state.borrow_mut::<ProtocolContext>();
    let host = JsHostRuntime::new(&mut protocol.host);
    let fut = process_and_dispatch_request(
        host,
        protocol.tx.clone(),
        protocol.address.clone().into(),
        method,
        url.clone(),
        headers,
        body,
    );
    let fetch_request_resource = FetchRequestResource {
        future: Box::pin(fut),
        url,
        from: protocol.address.clone(),
    };
    let request_rid = state.resource_table.add(fetch_request_resource);
    Ok(FetchReturn {
        request_rid,
        cancel_handle_rid: None,
    })
}

/// Dispatch the request to the appropriate handler based on the scheme and always
/// returns a response.
///
/// A new transaction snapshot is created before dispatching the run function and
/// committed/rolledback when it completes.
///
/// Callers should not process the response further other than converting it into
/// the expected response type.This function is agnostic of the context in which it
/// is called thus suitable as the [`crate::operation::RunFunction`] handler
pub async fn process_and_dispatch_request(
    host: JsHostRuntime<'static>,
    mut tx: Transaction,
    from: Address,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
) -> Response {
    let scheme = SupportedScheme::try_from(&url);
    match scheme {
        Ok(SupportedScheme::Jstz) => {
            let mut host = host;
            let mut is_successful = true;
            tx.begin();
            let result = dispatch_run(
                &mut host,
                &mut tx,
                from,
                method,
                url,
                headers,
                data,
                &mut is_successful,
            )
            .await;
            let _ = commit_or_rollback(&mut host, &tx, is_successful && result.is_ok());
            result.into()
        }
        Err(err) => err.into(),
    }
}

/// # Safety
/// Transaction snapshot creation and commitment should happen outside this function
async fn dispatch_run(
    host: &mut impl HostRuntime,
    tx: &mut Transaction,
    from: Address,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
    is_successful: &mut bool,
) -> Result<Response> {
    let to: Address = (&url).try_into()?;
    let mut headers = process_headers_and_transfer(tx, host, headers, &from, &to)?;
    headers.push((REFERRER_HEADER_KEY.clone(), from.to_base58().into()));
    match to.kind() {
        AddressKind::User => todo!(),
        AddressKind::SmartFunction => {
            let address = to.as_smart_function().unwrap();
            let run_result = HostScript::load_and_run(
                host,
                tx,
                address.clone(),
                method,
                url.clone(),
                headers,
                data,
            )
            .await;
            if let Ok(response) = run_result {
                if response.status < 200 || response.status >= 300 {
                    // Anything not a success should rollback
                    *is_successful = false;
                    clean_and_validate_headers(response.headers).map(
                        |ProcessedHeaders { headers, .. }| Response {
                            headers,
                            ..response
                        },
                    )
                } else {
                    let to: Address = (&url).try_into()?;
                    let headers = process_headers_and_transfer(
                        tx,
                        host,
                        response.headers,
                        &to,
                        &from,
                    )?;
                    Ok(Response {
                        headers,
                        ..response
                    })
                }
            } else {
                run_result
            }
        }
    }
}

async fn fetch_send(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
) -> Result<deno_fetch_base::FetchResponse> {
    let request = state
        .borrow_mut()
        .resource_table
        .take::<FetchRequestResource>(rid)
        .unwrap();

    let request = Rc::try_unwrap(request)
        .ok()
        .expect("multiple op_fetch_send ongoing");

    let response = request.future.await;
    let response_rid = state
        .borrow_mut()
        .resource_table
        .add(FetchResponseResource {
            body: RefCell::new(Some(response.body)),
        });

    Ok(deno_fetch_base::FetchResponse {
        status: response.status,
        status_text: response.status_text,
        headers: response.headers,
        url: request.url.to_string(),
        response_rid,
        content_length: None,
        remote_addr_ip: None,
        remote_addr_port: None,
        error: None,
    })
}

#[derive(Default)]
struct ProcessedHeaders {
    headers: Vec<(ByteString, ByteString)>,
    transfer: Option<NonZeroU64>,
}

/// Cleans headers and validates Transfer header if any
fn clean_and_validate_headers(
    headers: Vec<(ByteString, ByteString)>,
) -> Result<ProcessedHeaders> {
    let mut processed = ProcessedHeaders {
        headers: Vec::with_capacity(headers.len() + 2),
        transfer: None,
    };
    for (mut key, value) in headers {
        key.make_ascii_lowercase();
        let key_slice = key.trim_ascii();
        // Set transfer or error if already set
        if key_slice.eq_ignore_ascii_case(TRANSFER_HEADER_KEY.as_slice()) {
            if processed.transfer.is_none() {
                let value = value.to_vec();
                let value = String::from_utf8_lossy(value.as_slice());
                processed.transfer = Some(
                    NonZeroU64::from_str(&value)
                        .map_err(|_| FetchError::InvalidHeaderType)?,
                )
            } else {
                Err(FetchError::InvalidHeaderType)?;
            }
        }
        // Remove keys that shouldn' be there and might cause confusion
        else if !(key_slice.eq_ignore_ascii_case(REFERRER_HEADER_KEY.as_slice())
            || key_slice.starts_with(EXTENSION_PREFIX_HEADER_KEY.as_slice()))
        {
            processed.headers.push((key, value));
        }
    }
    Ok(processed)
}

static REFERRER_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("referrer"));
static AMOUNT_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-amount"));
static TRANSFER_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-transfer"));
static EXTENSION_PREFIX_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz"));

/// - performs transfers if `x-jstz-transfer` is present
/// - adds `x-jstz-amount` with transferred amount if any
fn process_headers_and_transfer(
    tx: &mut Transaction,
    host: &mut impl HostRuntime,
    headers: Vec<(ByteString, ByteString)>,
    from: &impl Addressable,
    to: &impl Addressable,
) -> Result<Vec<(ByteString, ByteString)>> {
    let mut processed_headers = clean_and_validate_headers(headers)?;
    if let Some(amount) = processed_headers.transfer {
        Account::transfer(host, tx, from, to, amount.into())
            .map_err(|e| FetchError::JstzError(e.to_string()))?;
        processed_headers.headers.push((
            AMOUNT_HEADER_KEY.clone(),
            ByteString::from(amount.to_string()),
        ));
    }
    Ok(processed_headers.headers)
}

fn commit_or_rollback(
    host: &mut impl HostRuntime,
    tx: &Transaction,
    is_success: bool,
) -> Result<()> {
    let result = if is_success {
        tx.commit(host)
    } else {
        tx.rollback()
    };
    result.map_err(|e| FetchError::JstzError(e.to_string()))
}
