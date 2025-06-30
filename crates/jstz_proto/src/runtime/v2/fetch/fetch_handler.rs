use crate::logger::{
    log_request_end_with_host, log_request_start_with_host, log_response_status_code,
};
use crate::operation::OperationHash;
use crate::runtime::v2::fetch::error::{FetchError, Result};
use crate::runtime::v2::fetch::http::Request;
use crate::runtime::v2::ledger;
use crate::runtime::v2::protocol_context::PROTOCOL_CONTEXT;

use deno_core::{
    resolve_import, v8, ByteString, JsBuffer, OpState, ResourceId, StaticModuleLoader,
};
use deno_fetch_base::{FetchHandler, FetchResponse, FetchReturn};
use futures::FutureExt;
use jstz_crypto::public_key_hash::PublicKeyHash;
use std::future::Future;
use std::pin::Pin;
use std::{cell::RefCell, rc::Rc};

use jstz_core::host::JsHostRuntime;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_runtime::sys::{
    FromV8, Headers as JsHeaders, Request as JsRequest, RequestInit as JsRequestInit,
    Response as JsResponse, ToV8,
};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
use url::Url;

use crate::context::account::{Account, Address, AddressKind, Addressable};
use crate::runtime::v2::fetch::resources::FetchRequestResource;
use deno_fetch_base::FetchResponseResource;

use super::host_script::HostScript;
use super::http::HostName;
use super::http::{Body, Response, SupportedScheme};
use std::num::NonZeroU64;
use std::str::FromStr;

/// Provides the backend for Deno's [fetch](https://docs.deno.com/api/web/~/fetch) which structures
/// its implementation into two steps to allow an [abort handler](https://github.com/jstz-dev/deno/blob/v2.1.10-jstz/ext/fetch_base/26_fetch.js#L182)
/// to be registered in JS in between the two routines
///
/// 1. [`fetch`] creates a future that will dispatch the request to the appropriate handler depending
///    on the scheme then store the future in the Resource table
/// 2. [`fetch_send`] awaits for the future to complete then returns the response with its body hidden
///    behind a Resource which allows the body to be consumed as an [async JS ReadableStream](https://developer.mozilla.org/en-US/docs/Web/API/Streams_API/Using_readable_streams#consuming_a_fetch_using_asynchronous_iteration)
///
/// Unlike smart function calls in the V1 runtime, [`JstzFetchHandler`] decouples succesful Transaction from
/// unsuccessful Responses. This allows callees to send transfers within the header of an error response. However,
/// there are a few things to be mindful off in its current state
///
/// 1. State updates, including transfers, can only be rolledback when the smart function throws an error. A
///    dedicate `abort` API is necessary to perform this more gracefully
/// 2. Although the  `fetch` API is asynchronous, Transactions are not. Trying to `fetch` two smart functions
///    concurrently is Undefined Behaviour
///
/// Current behaviour
///
/// * Calling conveniton
///     - `fetch` should target a `jstz` schemed URL with host referencing a valid Smart Function address (callee)
///     -  The callee smart function should export a default hander that accepts a Request and returns a Response
/// *  Header hygiene in Request/Response
///     - The "referer" header key will be set to/replaced with the caller's address
///     - "x-jstz-*" header keys will be removed if present except valid header "x-jstz-transfer"
/// *. Header transfer
///     - If the "x-jstz-transfer: <amount>" header key is present, the protocol will attempt to transfer <amount> from caller to callee.
///       If successful, the "x-jstz-transfer" key will be replaced by "x-jstz-amount". If not, the callee will returns an error Response.
///       Header transfers also apply to Responses but from callee to caller.
/// * Transaction
///     - A new transaction snapshot is created before running the callee's handler and committed/rolledback after it completes
/// * Errors
///      - If the callee's script throws an uncaught eror, `fetch` will automatically wrap it into a 500 InternalServerError and
///        the transaction is rolled back
///      - If the callee's script returns 200 < code <= 300 Response, the headers will be cleansed of unexpected headers and the transaction
///        rolled back. That is, anything that isn't a success isn't expected to update state.
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
    let (tx, from, host) = {
        let rt_context = state.borrow_mut::<RuntimeContext>();
        (
            rt_context.tx.clone(),
            rt_context.address.clone(),
            JsHostRuntime::new(&mut rt_context.host),
        )
    };
    let SourceAddress(source) = state.borrow::<SourceAddress>();
    let fut = process_and_dispatch_request(
        host,
        tx,
        false,
        None,
        source.clone(),
        from.clone().into(),
        method,
        url.clone(),
        headers,
        body,
    );
    let fetch_request_resource = FetchRequestResource {
        future: Box::pin(fut),
        url,
        from: from.clone(),
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
#[allow(clippy::too_many_arguments)]
pub async fn process_and_dispatch_request(
    mut host: JsHostRuntime<'static>,
    mut tx: Transaction,
    is_run_function: bool,
    // Top level operation hash
    operation_hash: Option<OperationHash>,
    // Source address that initiated the RunFunction operation. Must be a user address
    source: Address,
    // Address that initiated the call. This will be a user address equal to source
    // if called from RunFunction or a smart function address if called from fetch
    from: Address,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
) -> Response {
    let scheme = SupportedScheme::try_from(&url);
    let source = match SourceAddress::try_from(source) {
        Ok(ok) => ok,
        Err(e) => return e.into(),
    };
    let response = match scheme {
        Ok(SupportedScheme::Jstz) => {
            let mut is_successful = true;
            tx.begin();
            let result = dispatch_run(
                &mut host,
                &mut tx,
                is_run_function,
                operation_hash.as_ref(),
                source,
                from,
                method,
                &url,
                headers,
                data,
                &mut is_successful,
            )
            .await;
            let _ =
                commit_or_rollback(&mut host, &mut tx, is_successful && result.is_ok());
            result.into()
        }
        Ok(SupportedScheme::Http) => {
            match dispatch_oracle(
                &mut host,
                &mut tx,
                is_run_function,
                source,
                method,
                &url,
                headers,
                data,
            ) {
                Ok(resp) => resp.await,
                Err(e) => Err(e).into(),
            }
        }
        Err(err) => err.into(),
    };
    log_event(
        &mut host,
        operation_hash.as_ref(),
        LogEvent::Response((&url, &response)),
    );
    response
}

fn dispatch_oracle(
    host: &mut JsHostRuntime<'static>,
    tx: &mut Transaction,
    is_run_function: bool,
    source: SourceAddress,
    method: ByteString,
    url: &Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
) -> Result<Pin<Box<dyn Future<Output = Response>>>> {
    if is_run_function {
        return Ok(async {
            Response {
                status: 400,
                status_text: "Bad Request".into(),
                headers: Vec::with_capacity(0),
                body: "HTTP requests are not callable from RunFunction".into(),
            }
        }
        .boxed_local());
    }
    let response_rx = {
        let oracle_ctx = PROTOCOL_CONTEXT
            .get()
            .expect("Protocol context should be initialized")
            .oracle();
        let mut oracle = oracle_ctx.lock();
        oracle.send_request(
            host,
            tx,
            &source.as_user(),
            Request {
                method,
                url: url.clone(),
                headers,
                body: data,
            },
        )
    }?;
    Ok(async {
        match response_rx.await {
            Ok(resp) => resp,
            Err(_cancelled) => Response {
                status: 408,
                status_text: "Request Timeout".to_string(),
                headers: Vec::with_capacity(0),
                body: Body::zero_capacity(),
            },
        }
    }
    .boxed_local())
}

/// # Safety
/// Transaction snapshot creation and commitment should happen outside this function
async fn dispatch_run(
    host: &mut JsHostRuntime<'static>,
    tx: &mut Transaction,
    is_run_function: bool,
    operation_hash: Option<&OperationHash>,
    source: SourceAddress,
    from: Address,
    method: ByteString,
    url: &Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
    is_successful: &mut bool,
) -> Result<Response> {
    let to = url.try_into();
    match to {
        Ok(HostName::Address(to)) => {
            log_event(host, operation_hash, LogEvent::RequestStart(&to));
            let response = handle_address(
                host,
                tx,
                operation_hash,
                source,
                to.clone(),
                method,
                url,
                headers,
                data,
                is_successful,
                from,
            )
            .await;
            log_event(host, operation_hash, LogEvent::RequestEnd(&to));
            response
        }
        Ok(HostName::JstzHost) if is_run_function => Ok(Response {
            status: 400,
            status_text: "Bad Request".into(),
            headers: Vec::with_capacity(0),
            body: "HostScript is not callable from RunFunction".into(),
        }),
        Ok(HostName::JstzHost) => HostScript::route(host, tx, from, method, url).await,
        Err(e) => Err(e),
    }
}

async fn handle_address(
    host: &mut JsHostRuntime<'static>,
    tx: &mut Transaction,
    operation_hash: Option<&OperationHash>,
    source: SourceAddress,
    to: Address,
    method: ByteString,
    url: &Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
    is_successful: &mut bool,
    from: Address,
) -> Result<Response> {
    let mut headers = process_headers_and_transfer(tx, host, headers, &from, &to)?;
    headers.push((REFERER_HEADER_KEY.clone(), from.to_base58().into()));
    let response = match to.kind() {
        AddressKind::User => Ok(Response {
            status: 200,
            status_text: "OK".into(),
            headers,
            body: Body::Vector(Vec::with_capacity(0)),
        }),
        AddressKind::SmartFunction => {
            if !Account::exists(host, tx, &to)
                .map_err(|e| FetchError::JstzError(e.to_string()))?
            {
                return Ok(Response {
                    status: 404,
                    status_text: "Not Found".to_string(),
                    headers,
                    body: "Account does not exist".into(),
                });
            }
            let address = to.as_smart_function().unwrap();
            let run_result = load_and_run(
                host,
                tx,
                operation_hash,
                source,
                address.clone(),
                method,
                url,
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
                    let to: Address = url.try_into()?;
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
    };
    response
}

// - Loads the smart function script at `address`
// - Bootstraps a new runtime with new context and module loader
// - Runs the smart function
async fn load_and_run(
    host: &mut impl HostRuntime,
    tx: &mut Transaction,
    operation_hash: Option<&OperationHash>,
    source: SourceAddress,
    address: SmartFunctionHash,
    method: ByteString,
    url: &Url,
    headers: Vec<(ByteString, ByteString)>,
    body: Option<Body>,
) -> Result<Response> {
    let mut body = body;

    // 0. Prepare Protocol
    let mut proto = RuntimeContext::new(
        host,
        tx,
        address.clone(),
        operation_hash.map(|v| v.to_string()).unwrap_or_default(),
    );
    // 1. Load script
    let script = { load_script(tx, &mut proto.host, &proto.address)? };
    // 2. Prepare runtime
    let path = format!("jstz://{}", address);
    // `resolve_import` will panic without pinning
    let path = std::pin::Pin::new(path.as_str());
    let specifier = resolve_import(&path, "").unwrap();
    // TODO: Investigate if its possible to replace moodule loader with explicit module loading
    // from raw, precompiled or cached script
    let module_loader = StaticModuleLoader::with(specifier.clone(), script);
    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        module_loader: Rc::new(module_loader),
        fetch: deno_fetch_base::deno_fetch::init_ops_and_esm::<ProtoFetchHandler>(()),
        protocol: Some(proto),
        extensions: vec![ledger::jstz_ledger::init_ops_and_esm()],
        ..Default::default()
    });
    runtime.set_state(source);

    // 3. Prepare request
    let request = {
        let scope = &mut runtime.handle_scope();
        let headers = JsHeaders::new_with_sequence(scope, headers.into())?;
        let request_init = JsRequestInit::new(scope);
        request_init.set_headers(scope, headers)?;
        request_init.set_method(scope, method.into())?;
        if let Some(body) = body.take() {
            let body = body.to_v8(scope)?;
            request_init.set_body(scope, body)?;
        }
        let request =
            JsRequest::new_with_string_and_init(scope, url.to_string(), request_init)?;
        let request = request.to_v8(scope)?;
        v8::Global::new(scope, request)
    };

    // 4. Run
    let args = [request];
    let id = runtime.execute_main_module(&specifier).await?;
    let result = runtime.call_default_handler(id, &args).await?;
    let response = convert_js_to_response(&mut runtime, result)
        .await
        .map_err(|_| FetchError::InvalidResponseType)?;
    Ok(response)
}

fn load_script(
    tx: &mut Transaction,
    host: &impl HostRuntime,
    address: &SmartFunctionHash,
) -> Result<String> {
    let code = Account::function_code(host, tx, address)
        .map(|s| s.to_string())
        .map_err(|err| FetchError::JstzError(err.to_string()))?;
    if code.is_empty() {
        return Err(FetchError::EmptyCode {
            address: address.clone(),
        });
    }
    Ok(code)
}

async fn convert_js_to_response(
    runtime: &mut JstzRuntime,
    value: v8::Global<v8::Value>,
) -> Result<Response> {
    let (mut response, body) = {
        let scope = &mut runtime.handle_scope();
        let local_value = v8::Local::new(scope, value);
        let response = <JsResponse as FromV8>::from_v8(scope, local_value)?;
        let headers: Vec<(ByteString, ByteString)> = response
            .headers(scope)?
            .entries(scope)?
            .iter(scope)
            .collect();
        let status = response.status(scope)?;
        let status_text = response.status_text(scope)?;
        let body = response.array_buffer(scope)?;
        let response = Response {
            status,
            status_text,
            headers,
            body: Body::Vector(Vec::with_capacity(0)),
        };
        (response, body)
    };

    let body = Body::Buffer(body.with_runtime(runtime).await?);
    response.body = body;
    Ok(response)
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
    let body = response.body;
    let body_size = body.len() as u64;
    let response_rid = state
        .borrow_mut()
        .resource_table
        .add(FetchResponseResource::<Body>::new(body, Some(body_size)));

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
        else if !(key_slice.eq_ignore_ascii_case(REFERER_HEADER_KEY.as_slice())
            || key_slice.starts_with(EXTENSION_PREFIX_HEADER_KEY.as_slice()))
        {
            processed.headers.push((key, value));
        }
    }
    Ok(processed)
}

// "Referer" is mispelt in the HTTP spec
static REFERER_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("referer"));
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

enum LogEvent<'a> {
    RequestStart(&'a Address),
    RequestEnd(&'a Address),
    Response((&'a Url, &'a Response)),
}

fn log_event(
    host: &mut JsHostRuntime<'static>,
    op_hash: Option<&OperationHash>,
    event: LogEvent,
) {
    if let Some(op) = op_hash {
        match event {
            LogEvent::RequestStart(address) => {
                if let Address::SmartFunction(smart_function_addr) = &address {
                    log_request_start_with_host(
                        host,
                        smart_function_addr.clone(),
                        op.to_string(),
                    )
                }
            }
            LogEvent::RequestEnd(address) => {
                if let Address::SmartFunction(smart_function_addr) = &address {
                    log_request_end_with_host(
                        host,
                        smart_function_addr.clone(),
                        op.to_string(),
                    )
                }
            }
            LogEvent::Response((url, res)) => {
                log_response_status_code(host, &url, op.to_string(), res.status)
            }
        }
    }
}

// Newtype used to store source in op state. Always a user address
struct SourceAddress(Address);

impl SourceAddress {
    pub fn as_user(&self) -> &PublicKeyHash {
        self.0.as_user().unwrap()
    }
}

impl TryFrom<Address> for SourceAddress {
    type Error = FetchError;

    fn try_from(source: Address) -> std::result::Result<Self, Self::Error> {
        if matches!(source, Address::User(_)) {
            Ok(SourceAddress(source))
        } else {
            Err(FetchError::InvalidSourceAddress)
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, str::FromStr};

    use deno_core::{resolve_import, StaticModuleLoader};

    use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};

    use jstz_core::{
        host::JsHostRuntime,
        kv::{Storage, Transaction},
    };
    use jstz_crypto::{
        hash::{Blake2b, Hash},
        public_key::PublicKey,
        smart_function_hash::SmartFunctionHash,
    };
    use jstz_utils::test_util::TOKIO;

    use serde_json::{json, Value as JsonValue};
    use url::Url;

    use super::ProtoFetchHandler;
    use crate::runtime::ParsedCode;
    use crate::{
        context::account::{Account, Address},
        tests::DebugLogSink,
    };
    use crate::{
        event,
        runtime::v2::{
            fetch::fetch_handler::process_and_dispatch_request, oracle::OracleRequest,
            protocol_context::ProtocolContext,
        },
    };
    use crate::{
        runtime::v2::{
            fetch::{fetch_handler::SourceAddress, http::Response},
            protocol_context::PROTOCOL_CONTEXT,
            test_utils::*,
        },
        storage::ORACLE_PUBLIC_KEY_PATH,
    };

    use std::rc::Rc;

    // Script simply fetches the smart function given in the path param
    // eg. jstz://<host address>/<remote address> will call fetch("jstz://<remote address>")
    const SIMPLE_REMOTE_CALLER: &str = "export default async (req) => await fetch(`jstz://${new URL(req.url).pathname.substring(1)}`)";

    // Run behaviour

    // Fetch with `jstz` scheme runs a smart function.
    #[test]
    fn fetch_runs_smart_function() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => new Response("hello world")"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                false,
                None,
                source_address.clone().into(),
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                "hello world",
                String::from_utf8(response.body.into()).unwrap()
            );
        });
    }

    // Fetch rejects unsupported schemes runs a smart function.
    #[test]
    fn fetch_rejects_unsupported_scheme() {
        TOKIO.block_on(async {

        // Code
        let run = "export default async (req) => await fetch(`tezos://${new URL(req.url).pathname.substring(1)}`)";

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (host, tx, source_address, hashes) = setup(&mut host, [run]);
        let run_address = hashes[0].clone();

        // Run
        let response = process_and_dispatch_request(
            host,
            tx,
            false,
            None,
            source_address.clone().into(),
            source_address.into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, run_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        // Assert
        assert_eq!(response.status, 500);
        assert_eq!(response.status_text, "InternalServerError");
        assert_eq!(
            json!({"class":"TypeError","message":"Unsupport scheme 'tezos'"}),
            serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                .unwrap()
        );
    });
    }

    // Fetch rejects unsupported schemes runs a smart function.
    #[test]
    fn fetch_rejects_unsupported_address_scheme() {
        TOKIO.block_on(async {
            // Code
            let run = "export default async (req) => await fetch(`jstz://abc123`)";

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                false,
                None,
                source_address.clone().into(),
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(response.status, 500);
            assert_eq!(response.status_text, "InternalServerError");
            assert_eq!(
                json!({"class":"RuntimeError","message":"InvalidAddress"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            );
        });
    }

    // Smart functions must return a Response if successfully ran
    #[test]
    fn smart_function_must_return_response() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {}"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                false,
                None,
                source_address.clone().into(),
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!("InternalServerError", response.status_text);
            assert_eq!(500, response.status);
            assert_eq!(
                json!({"class": "TypeError","message":"Invalid Response type"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            );
        });
    }

    #[test]
    fn fetch_supports_empty_response_body() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => new Response()"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();
            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                false,
                None,
                source_address.clone().into(),
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let body: Vec<u8> = response.body.into();
            assert!(body.is_empty());
        })
    }

    // Global changes are isolated between smart function calls
    #[test]
    fn fetch_provides_isolation() {
        TOKIO.block_on(async {
            // Code
            let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            globalThis.leakyState = "abc"
            return await fetch(`jstz://${address}`)
        }"#;
            let remote = r#"export default async (_req) => {
            if (globalThis.leakyState ===  "abc") {  throw new Error("leak detected!"); }
            return new Response("hello world")
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                false,
                None,
                source_address.clone().into(),
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                "hello world",
                String::from_utf8(response.body.into()).unwrap()
            )
        });
    }

    // Fetch can be called recursively (re-entrant)
    // FIXME: Smart functions should not be re-entrant by default
    #[test]
    fn fetch_recursive() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/recursive/run.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();

            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx,
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            let json =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "count": 3
                }),
                json
            )
        })
    }

    // Racing multiple fetch calls is awaitable at different points of the program
    #[test]
    fn fetch_raceable() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/raceable/run.js");
            let remote = include_str!("tests/resources/raceable/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx,
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            let json =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "data": 3
                }),
                json
            )
        });
    }

    // The default behaviour of deno async is to run eagerly, even when not awaited on, for
    // latency reasons. This means that side effects like KV updates and transfers are performed
    // when the execution completes successfully even when not awaited on
    #[test]
    fn fetch_eagerly_executes() {
        TOKIO.block_on(async {
            // Code
            let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            fetch(`jstz://${address}/5`)
            fetch(`jstz://${address}/-3`)
            return new Response()
        }"#;
            let remote = r#"export default async (req) => {
            let incr = Number.parseInt(new URL(req.url).pathname.substring(1));
            let value = Kv.get("value") ?? 0;
            Kv.set("value", value + incr);
            return new Response()
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::ext::jstz_kv::kv::Kv::new(remote_address.to_string());
            let result = kv
                .get(&mut host, &mut tx, "value")
                .unwrap()
                .unwrap()
                .0
                .clone();
            assert_eq!(2, serde_json::from_value::<usize>(result).unwrap());
        });
    }

    // Headers processing behaviour

    #[test]
    fn fetch_default_headers() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
            let body = Object.fromEntries(req.headers.entries());
            return new Response(JSON.stringify(body))
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let request_headers =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert_eq!(
                json!({
                    "accept":"*/*",
                    "accept-language":"*",
                    "referer":"KT1WEAA8whopt6FqPodVErxnQysYSkTan4wS"
                }),
                request_headers
            );

            let response_headers: HashMap<String, String> = response
                .headers
                .into_iter()
                .map(|(k, v)| {
                    (
                        String::from_utf8(k.as_slice().to_vec()).unwrap(),
                        String::from_utf8(v.as_slice().to_vec()).unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                json!({
                    "content-type":"text/plain;charset=UTF-8",
                }),
                serde_json::to_value(response_headers).unwrap()
            );
        })
    }

    #[test]
    fn request_header_has_referer() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
            let body = Object.fromEntries(req.headers.entries());
            return new Response(JSON.stringify(body))
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            let request_headers =
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap();
            assert!(request_headers["referer"] == run_address.to_string());
        })
    }

    #[test]
    fn fetch_replaces_referer_in_request_header() {
        TOKIO.block_on(async {

        // Code
        let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            let request = new Request(`jstz://${address}`, {
                headers: {
                    Referer: req.headers.get("referer") // Tries to forward referer
                }
            });
            return await fetch(request)
        }"#;
        let remote =
            r#"export default async (req) => new Response(req.headers.get("referer"))"#;

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
        let run_address = hashes[0].clone();
        let remote_address = hashes[1].clone();

        // Run
        let response = process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx.clone(),
            false,
            None,
            jstz_mock::account1().into(),
            jstz_mock::account1().into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        assert_eq!(
            run_address.to_string(),
            String::from_utf8(response.body.to_vec()).unwrap()
        );
    })
    }

    #[test]
    fn transfer_succeeds() {
        TOKIO.block_on(async {
            // Code
            let run = include_str!("tests/resources/transfer_succeeds/run.js");
            let remote = include_str!("tests/resources/transfer_succeeds/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert!(response.status == 200);
            assert_eq!(
                8_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                2_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn transfer_fails_when_error_thrown() {
        TOKIO.block_on(async {
            let run =
                include_str!("tests/resources/transfer_fails_when_error_thrown/run.js");
            let remote = include_str!(
                "tests/resources/transfer_fails_when_error_thrown/remote.js"
            );

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(
                10_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                0,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn fetch_cleans_headers() {
        TOKIO.block_on(async {
            let run = include_str!("tests/resources/fetch_cleans_headers/run.js");
            let remote = include_str!("tests/resources/fetch_cleans_headers/remote.js");

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;
            assert_eq!(response.status, 200);
            assert_eq!(
                9_000_000,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                1_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        })
    }

    #[test]
    fn transfer_rejects_when_invalid() {
        TOKIO.block_on(async {
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"
            export default async (req) => {
                return new Response(null, {
                    headers: {
                        "X-JSTZ-TRANSFER": 100
                    }
                })
            }
        "#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(500, response.status);
            assert_eq!(
                json!({"class":"RuntimeError","message":"InsufficientFunds"}),
                serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                    .unwrap()
            )
        })
    }

    #[test]
    fn transfer_fails_when_status_not_2xx() {
        TOKIO.block_on(async {
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"
            export default async (req) => {
                return new Response(null, {
                    status: 400,
                    headers: {
                        "X-JSTZ-TRANSFER": 4000000
                    }
                })
            }
        "#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(&mut host, &mut tx, &remote_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(400, response.status);
            assert_eq!(
                0,
                Account::balance(&mut host, &mut tx, &run_address).unwrap()
            );
            assert_eq!(
                10_000_000,
                Account::balance(&mut host, &mut tx, &remote_address).unwrap()
            );
        });
    }

    // Transaction behaviour

    #[test]
    fn transaction_rolled_back_when_error_thrown() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {
            Kv.set("test", 123)
            throw new Error("boom")
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = crate::runtime::Kv::new(remote_address.to_string());
            let mut tx = tx;
            let result = kv.get(&mut host, &mut tx, "test").unwrap();
            assert!(result.is_none())
        });
    }

    #[test]
    fn transaction_rolled_back_when_status_not_2xx() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (_req) => {
            Kv.set("test", 123)
            return new Response(null, { status: 500 })
        }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = crate::runtime::Kv::new(remote_address.to_string());
            let mut tx = tx;
            let result = kv.get(&mut host, &mut tx, "test").unwrap();
            assert!(result.is_none())
        });
    }

    // Error behaviour

    // Errors that are a result of evaluating the request (server side issues) are converted
    // into an error response
    #[test]
    fn error_during_sf_execution_converts_to_error_response() {
        TOKIO.block_on(async {

        // Code
        let run = SIMPLE_REMOTE_CALLER;
        let remote = r#"export default async (_req) => {
            throw new Error("boom");
        }"#;

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
        let run_address = hashes[0].clone();
        let remote_address = hashes[1].clone();

        // Run
        let response = process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx.clone(),
            false,
            None,
            jstz_mock::account1().into(),
            jstz_mock::account1().into(),
            "GET".into(),
            Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                .unwrap(),
            vec![],
            None,
        )
        .await;

        assert_eq!("InternalServerError", response.status_text);
        assert_eq!(500, response.status);
        assert_eq!(
            json!({"class":"RuntimeError","message":"Error: boom\n    at default (jstz://KT1WSFFotGccKa4WZ5PNQGT3EgsRutzLMD4z:2:19)"}),
            serde_json::from_slice::<JsonValue>(response.body.to_vec().as_slice())
                .unwrap()
        );
    });
    }

    #[tokio::test]
    async fn test_fetch_response_body_stream() {
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let address =
            SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                .unwrap();

        let mut tx = jstz_core::kv::Transaction::default();
        tx.begin();
        let protocol = Some(RuntimeContext::new(
            &mut host,
            &mut tx,
            address.clone(),
            String::new(),
        ));

        let source = Address::User(jstz_mock::account1());
        let fetched_script = r#"
            const handler = async (req) => {
                let reqBody = await req.arrayBuffer();
                return new Response(reqBody);
            }
            export default handler;
        "#;

        Account::add_balance(&mut host, &mut tx.clone(), &source, 10000)
            .expect("add balance");

        let func_addr = Account::create_smart_function(
            &mut host,
            &mut tx,
            &source,
            100,
            ParsedCode(fetched_script.to_string()),
        )
        .unwrap();

        drop(tx);

        let code = format!(
            r#"

            const call = async () => {{
                const body = [1,2,3,4,5,6,7,8,9,10];
                const expectedBytes = body.length;
                // 1. test byob mode
                let request = new Request("jstz://{func_addr}", {{
                    method: "POST",
                    body: new Uint8Array(body),
                }})
                let response = await fetch(request);
                const CHUNK_SIZE = 3;
                let actualBytes = 0;
                let count = 0;
                let reader = response.body.getReader({{ mode: "byob" }});
                while (true) {{
                    const buf = new Uint8Array(CHUNK_SIZE);
                    const {{ value, done }} = await reader.read(buf);
                    if (done) break;
                    actualBytes += value.byteLength;
                    count += 1;
                }}
                if (actualBytes !== expectedBytes) {{
                    throw new Error("size is incorrect");
                }}
                const expectedCount = Math.floor(actualBytes / CHUNK_SIZE) + 1;
                if (count !== expectedCount) {{
                    throw new Error("count is incorrect");
                }}
                // 2. test default mode
                request = new Request("jstz://{func_addr}", {{
                    method: "POST",
                    body: new Uint8Array(body),
                }})
                response = await fetch(request);
                reader = response.body.getReader();
                actualBytes = 0;
                // read all the body
                while (true) {{
                    const {{ value, done }} = await reader.read();
                    if (done) break;
                    actualBytes += value.byteLength;
                }}
                if (actualBytes !== expectedBytes) {{
                    throw new Error("size is incorrect");
                }}
                return response
            }}
            export default call;
        "#
        );

        let specifier =
            resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();

        let module_loader = StaticModuleLoader::with(specifier.clone(), code);
        let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
            protocol,
            fetch: deno_fetch_base::deno_fetch::init_ops_and_esm::<ProtoFetchHandler>(()),
            module_loader: Rc::new(module_loader),
            ..Default::default()
        });
        runtime.set_state(SourceAddress::try_from(source).unwrap());
        let id = runtime.execute_main_module(&specifier).await.unwrap();
        let _ = runtime.call_default_handler(id, &[]).await.unwrap();
    }

    #[test]
    fn log_event() {
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let sink = DebugLogSink::new();
        let buf = sink.content();
        host.set_debug_handler(sink);
        let mut rt = JsHostRuntime::new(&mut host);
        let address = Address::SmartFunction(jstz_mock::sf_account1());
        let op_hash = Blake2b::from(b"op_hash".as_ref());

        super::log_event(
            &mut rt,
            Some(&op_hash),
            super::LogEvent::RequestStart(&address),
        );
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        assert_eq!(
            log,
            r#"[JSTZ:SMART_FUNCTION:REQUEST_START] {"type":"Start","address":"KT1QgfSE4C1dX9UqrPAXjUaFQ36F9eB4nNkV","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
"#
        );
        buf.lock().unwrap().clear();

        super::log_event(
            &mut rt,
            Some(&op_hash),
            super::LogEvent::RequestEnd(&address),
        );
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        assert_eq!(
            log,
            r#"[JSTZ:SMART_FUNCTION:REQUEST_END] {"type":"End","address":"KT1QgfSE4C1dX9UqrPAXjUaFQ36F9eB4nNkV","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
"#
        );
        buf.lock().unwrap().clear();

        super::log_event(
            &mut rt,
            Some(&op_hash),
            super::LogEvent::Response((
                &Url::from_str("foo://bar").unwrap(),
                &super::super::http::Response {
                    status: 403,
                    status_text: String::default(),
                    headers: vec![],
                    body: crate::runtime::v2::fetch::http::Body::Vector(vec![]),
                },
            )),
        );
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        assert_eq!(
            log,
            r#"[JSTZ:RESPONSE] {"url":"foo://bar","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5","status_code":403}
"#
        );
        buf.lock().unwrap().clear();

        // should not log when operation hash is missing
        super::log_event(&mut rt, None, super::LogEvent::RequestEnd(&address));
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        assert_eq!(log, "");

        // RequestEnd should not log with user address
        let address = Address::User(jstz_mock::account1());
        super::log_event(
            &mut rt,
            Some(&op_hash),
            super::LogEvent::RequestEnd(&address),
        );
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        assert_eq!(log, "");
    }

    #[tokio::test]
    async fn log_request_execution() {
        let from = Address::User(jstz_mock::account1());
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let sink = DebugLogSink::new();
        let buf = sink.content();
        host.set_debug_handler(sink);
        let mut tx = Transaction::default();

        // This smart function uses FormData, which is not supported in v1 runtime but in v2 runtime.
        let code = format!(
            r#"
        const handler = async (request) => {{
            const f = new FormData();
            f.append("a", "b");
            f.append("c", "d");
            let output = "";
            for (const [k, v] of f) {{
                output += `${{k}}-${{v}};`;
            }}
            console.warn(output);
            return new Response(output);
        }};
        export default handler;
        "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let func_addr =
            Account::create_smart_function(&mut host, &mut tx, &from, 0, parsed_code)
                .unwrap();
        tx.commit(&mut host).unwrap();
        tx.begin();
        let response = super::process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx,
            false,
            Some(Blake2b::from(b"op_hash".as_ref())),
            from.clone(),
            from,
            "GET".into(),
            Url::parse(&format!("jstz://{}/", func_addr.to_base58_check())).unwrap(),
            vec![],
            None,
        )
        .await;

        let text = String::from_utf8(response.body.to_vec()).unwrap();
        assert_eq!(text, "a-b;c-d;");
        assert_eq!(response.status, 200);
        let log = String::from_utf8(buf.lock().unwrap().to_vec()).unwrap();
        #[cfg(feature = "kernel")]
        let expected = r#"[JSTZ:SMART_FUNCTION:REQUEST_START] {"type":"Start","address":"KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
[JSTZ:SMART_FUNCTION:LOG] {"address":"KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m","requestId":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5","level":"WARN","text":"a-b;c-d;\n"}
[JSTZ:SMART_FUNCTION:REQUEST_END] {"type":"End","address":"KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
[JSTZ:RESPONSE] {"url":"jstz://KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m/","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5","status_code":200}
"#;
        #[cfg(not(feature = "kernel"))]
        let expected = r#"[JSTZ:SMART_FUNCTION:REQUEST_START] {"type":"Start","address":"KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
[WARN] a-b;c-d;
[JSTZ:SMART_FUNCTION:REQUEST_END] {"type":"End","address":"KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5"}
[JSTZ:RESPONSE] {"url":"jstz://KT1My1St5BPVWXsmaRSp6HtKmMFd24HvDF2m/","request_id":"afc02a7556649a25c0583e9168e5e862bbefa19b79c41c34b3c0bca38b15a0f5","status_code":200}
"#;
        assert_eq!(log, expected);
    }

    // Host script behaviour
    #[test]
    fn handle_balance_endpoint() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
                const response = await fetch(`jstz://jstz/balances/self`);
                return response;
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(200, response.status);
            assert_eq!("OK", response.status_text);
            assert_eq!("0", String::from_utf8(response.body.to_vec()).unwrap());
        });
    }

    #[test]
    fn handle_balance_endpoint_specific_address() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
                const response = await fetch(`jstz://jstz/balances/${req.headers.get("referer")}`);
                return response;
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, _source_address, hashes) =
                setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Add some balance to the account
            let _ = Account::add_balance(&mut host, &mut tx, &run_address, 5_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(200, response.status);
            assert_eq!("OK", response.status_text);
            assert_eq!("5000000", String::from_utf8(response.body.to_vec()).unwrap());
        });
    }

    #[test]
    fn handle_balance_endpoint_invalid_address() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
                const response = await fetch(`jstz://jstz/balances/invalid_address`);
                return response;
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(400, response.status);
            assert_eq!("Bad Request", response.status_text);
            assert!(String::from_utf8(response.body.to_vec())
                .unwrap()
                .contains("InvalidAddress"));
        });
    }

    #[test]
    fn handle_balance_endpoint_invalid_method() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
                const response = await fetch(`jstz://jstz/balances/self`, { method: 'POST' });
                return response;
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(405, response.status);
            assert_eq!("Method Not Allowed", response.status_text);
            assert_eq!(
                "Only GET method is allowed",
                String::from_utf8(response.body.to_vec()).unwrap()
            );
        });
    }

    #[test]
    fn unsupported_host_endpoint_returns_404() {
        TOKIO.block_on(async {
            // Code
            let run = SIMPLE_REMOTE_CALLER;
            let remote = r#"export default async (req) => {
                const response = await fetch(`jstz://jstz/unsupported/path`);
                return response;
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, tx, _source_address, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                false,
                None,
                jstz_mock::account1().into(),
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            assert_eq!(404, response.status);
            assert_eq!("Not Found", response.status_text);
            assert_eq!(
                "Not Found",
                String::from_utf8(response.body.into()).unwrap()
            );
        });
    }

    // Oracle behaviour

    #[test]
    fn fetch_http_returns_response() {
        TOKIO.block_on(async {
            let code = r#"
        export default async () => {
            let result = await fetch("http://example.com")
            return result
        }
        "#;
            let debug_sink = DebugLogSink::new();
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            host.set_debug_handler(debug_sink.clone());
            let pk = PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap();
            Storage::insert(&mut host, &ORACLE_PUBLIC_KEY_PATH, &pk).unwrap();
            let (mut host, mut tx, source_address, hashes) = setup(&mut host, [code]);
            Account::add_balance(&mut host,&mut tx, &source_address, 0).unwrap();
            tx.commit(&mut host).unwrap();

            let run_address = hashes[0].clone();
            ProtocolContext::init_global(&mut host, 0).unwrap();
            tokio::pin! {
                let response_fut = process_and_dispatch_request(
                    JsHostRuntime::new(&mut host),
                    tx.clone(),
                    false,
                    None,
                    jstz_mock::account1().into(),
                    jstz_mock::account1().into(),
                    "GET".into(),
                    Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                    vec![],
                    None,
                );
            };
            let response = Response {
                status: 200,
                status_text: "OK".into(),
                headers: Vec::with_capacity(0),
                body: serde_json::to_vec(&json!({ "message": "this is a test message" }))
                    .unwrap()
                    .into(),
            };


            let fetch_response = tokio::select! {
                response = &mut response_fut => {
                    response
                }
                _ = async {
                    while debug_sink.str_content().is_empty() {
                        tokio::task::yield_now().await
                    }
                    let oracle_request =
                        event::decode_line::<OracleRequest>(debug_sink.lines().first().unwrap())
                            .unwrap();
                    assert_eq!(oracle_request.request.method, "GET".into());
                    assert_eq!(
                        oracle_request.request.url,
                        Url::parse("http://example.com").unwrap()
                    );
                    assert_eq!(oracle_request.caller, source_address);
                    let oracle_ctx = PROTOCOL_CONTEXT.get().unwrap().oracle();
                    let mut oracle = oracle_ctx.lock();

                    oracle
                        .respond(&mut host, oracle_request.id, response.clone())
                        .unwrap();
                } => {
                    response_fut.await
                }
            };
            assert_eq!(response, fetch_response);
        })
    }
}
