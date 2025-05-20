use deno_core::{
    resolve_import, serde_v8, v8, AsyncResult, BufView, ByteString, JsBuffer, OpState,
    Resource, ResourceId, StaticModuleLoader, ToJsBuffer,
};
use deno_error::JsErrorClass;
use deno_fetch_base::{FetchHandler, FetchResponse, FetchReturn};
use jstz_core::host::JsHostRuntime;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_runtime::sys::{
    FromV8, Headers as JsHeaders, Request as JsRequest, RequestInit as JsRequestInit,
    Response as JsResponse, ToV8,
};
use jstz_runtime::JstzRuntimeOptions;
use jstz_runtime::{error::RuntimeError, JstzRuntime, ProtocolContext};
use parking_lot::FairMutex as Mutex;
use serde::Serialize;
use std::borrow::Cow;
use std::future::Future;
use std::num::NonZeroU64;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use url::Url;

use crate::context::account::{Account, Address, AddressKind, Addressable};

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
///     - The "referrer" header key will be set to/replaced with the caller's address
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

type Result<T> = std::result::Result<T, FetchError>;

impl From<Result<Response>> for Response {
    fn from(result: Result<Response>) -> Self {
        match result {
            Ok(response) => response,
            Err(err) => err.into(),
        }
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
    tx: Arc<Mutex<Transaction>>,
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
            tx.lock().begin();
            let result = dispatch_run(
                &mut host,
                tx.clone(),
                from,
                method,
                url,
                headers,
                data,
                &mut is_successful,
            )
            .await;
            let _ = commit_or_rollback(
                &mut host,
                tx.clone(),
                is_successful && result.is_ok(),
            );
            result.into()
        }
        Err(err) => err.into(),
    }
}

/// # Safety
/// Transaction snapshot creation and commitment should happen outside this function
async fn dispatch_run(
    host: &mut impl HostRuntime,
    tx: Arc<Mutex<Transaction>>,
    from: Address,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<Body>,
    is_successful: &mut bool,
) -> Result<Response> {
    let to: Address = (&url).try_into()?;
    let mut headers =
        process_headers_and_transfer(tx.clone(), host, headers, &from, &to)?;
    headers.push((REFERRER_HEADER_KEY.clone(), from.to_base58().into()));
    match to.kind() {
        AddressKind::User => todo!(),
        AddressKind::SmartFunction => {
            let address = to.as_smart_function().unwrap();
            let run_result = load_and_run(
                host,
                tx.clone(),
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

// - Loads the smart function script at `address`
// - Bootstraps a new runtime with new context and module loader
// - Runs the smart function
async fn load_and_run(
    host: &mut impl HostRuntime,
    tx: Arc<Mutex<Transaction>>,
    address: SmartFunctionHash,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    body: Option<Body>,
) -> Result<Response> {
    let mut body = body;

    // 0. Prepare Protocol
    let mut proto = ProtocolContext::new(host, tx.clone(), address.clone());

    // 1. Load script
    let script = { load_script(tx.clone(), &mut proto.host, &proto.address)? };

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
        ..Default::default()
    });

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

static REFERRER_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("referrer"));
static AMOUNT_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-amount"));
static TRANSFER_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz-transfer"));
static EXTENSION_PREFIX_HEADER_KEY: std::sync::LazyLock<ByteString> =
    std::sync::LazyLock::new(|| ByteString::from("x-jstz"));

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

/// - performs transfers if `x-jstz-transfer` is present
/// - adds `x-jstz-amount` with transferred amount if any
fn process_headers_and_transfer(
    tx: Arc<Mutex<Transaction>>,
    host: &mut impl HostRuntime,
    headers: Vec<(ByteString, ByteString)>,
    from: &impl Addressable,
    to: &impl Addressable,
) -> Result<Vec<(ByteString, ByteString)>> {
    let mut processed_headers = clean_and_validate_headers(headers)?;
    if let Some(amount) = processed_headers.transfer {
        Account::transfer(host, &mut tx.lock(), from, to, amount.into())
            .map_err(|e| FetchError::JstzError(e.to_string()))?;
        processed_headers.headers.push((
            AMOUNT_HEADER_KEY.clone(),
            ByteString::from(amount.to_string()),
        ));
    }
    Ok(processed_headers.headers)
}

fn load_script(
    tx: Arc<Mutex<Transaction>>,
    host: &impl HostRuntime,
    address: &SmartFunctionHash,
) -> Result<String> {
    let mut tx = tx.lock();
    Account::function_code(host, &mut tx, address)
        .map(|s| s.to_string())
        .map_err(|err| FetchError::JstzError(err.to_string()))
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

fn commit_or_rollback(
    host: &mut impl HostRuntime,
    tx: Arc<Mutex<Transaction>>,
    is_success: bool,
) -> Result<()> {
    let mut tx = tx.lock();
    let result = if is_success {
        tx.commit(host)
    } else {
        tx.rollback()
    };
    result.map_err(|e| FetchError::JstzError(e.to_string()))
}

// HTTP structures

/// Response returned from a fetch or Smart Function run
#[derive(Debug)]
pub struct Response {
    status: u16,
    status_text: String,
    headers: Vec<(ByteString, ByteString)>,
    body: Body,
}

#[derive(Debug)]
pub enum Body {
    Vector(Vec<u8>),
    Buffer(JsBuffer),
}

impl Body {
    #[allow(unused)]
    pub fn to_vec(self) -> Vec<u8> {
        self.into()
    }

    pub fn zero_capacity() -> Self {
        Self::Vector(Vec::with_capacity(0))
    }
}

impl From<Body> for Vec<u8> {
    fn from(body: Body) -> Self {
        match body {
            Body::Vector(items) => items,
            Body::Buffer(js_buffer) => js_buffer.to_vec(),
        }
    }
}

pub enum SupportedScheme {
    Jstz,
}

impl TryFrom<&Url> for SupportedScheme {
    type Error = FetchError;

    fn try_from(value: &Url) -> Result<Self> {
        match value.scheme() {
            "jstz" => Ok(Self::Jstz),
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_string())),
        }
    }
}

impl TryFrom<&Url> for Address {
    type Error = FetchError;

    fn try_from(url: &Url) -> Result<Self> {
        let raw_address = url.host().ok_or(url::ParseError::EmptyHost)?;
        Address::from_base58(raw_address.to_string().as_str())
            .map_err(|err| FetchError::JstzError(err.to_string()))
    }
}

// Resources

pub struct FetchRequestResource {
    pub future: Pin<Box<dyn Future<Output = Response>>>,
    pub url: Url,
    pub from: SmartFunctionHash,
}

pub struct FetchResponseResource {
    body: RefCell<Option<Body>>,
}

impl Resource for FetchRequestResource {}

impl Resource for FetchResponseResource {
    fn read(self: Rc<Self>, _limit: usize) -> AsyncResult<BufView> {
        Box::pin(async move {
            if let Some(body) = self.body.borrow_mut().take() {
                return Ok(match body {
                    Body::Buffer(body) => BufView::from(body),
                    Body::Vector(body) => BufView::from(body),
                });
            }
            Ok(BufView::empty())
        })
    }
}

// Errors

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FetchError {
    #[class(type)]
    #[error("Invalid Header type")]
    InvalidHeaderType,
    #[class(type)]
    #[error("Unsupport scheme '{0}'")]
    UnsupportedScheme(String),
    #[class(uri)]
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[class(type)]
    #[error("Invalid Response type")]
    InvalidResponseType,
    #[class("RuntimeError")]
    #[error(transparent)]
    RuntimeError(#[from] RuntimeError),
    #[class(not_supported)]
    #[error("{0}")]
    NotSupported(&'static str),
    // TODO: Boa's JsClass errors are not Send safe. Once we remove boa, we
    // should be able to use crate::Error type directly
    #[class("RuntimeError")]
    #[error("{0}")]
    JstzError(String),
}

#[derive(Serialize)]
pub struct FetchErrorJsClass {
    class: Cow<'static, str>,
    message: Option<Cow<'static, str>>,
}

impl From<FetchError> for FetchErrorJsClass {
    fn from(value: FetchError) -> Self {
        Self {
            class: value.get_class(),
            message: Some(value.get_message()),
        }
    }
}

impl From<FetchError> for Response {
    fn from(err: FetchError) -> Self {
        let error_body: FetchErrorJsClass = err.into();
        let error = serde_json::to_vec(&error_body)
            .map(Body::Vector)
            .ok()
            .unwrap_or(Body::zero_capacity());
        Response {
            status: 500,
            status_text: "InternalServerError".to_string(),
            headers: Vec::with_capacity(0),
            body: error,
        }
    }
}

impl<'s> ToV8<'s> for Body {
    fn to_v8(
        self,
        scope: &mut v8::HandleScope<'s>,
    ) -> jstz_runtime::error::Result<v8::Local<'s, v8::Value>> {
        match self {
            Body::Vector(items) => {
                let to_buffer = ToJsBuffer::from(items);
                let value = serde_v8::to_v8(scope, to_buffer)?;
                Ok(value)
            }
            Body::Buffer(js_buffer) => js_buffer.to_v8(scope),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, sync::Arc};

    use jstz_core::{
        host::{HostRuntime, JsHostRuntime},
        kv::Transaction,
    };
    use jstz_crypto::{
        public_key_hash::PublicKeyHash, smart_function_hash::SmartFunctionHash,
    };
    use jstz_utils::TOKIO;

    use parking_lot::FairMutex as Mutex;
    use serde_json::{json, Value as JsonValue};
    use url::Url;

    use super::process_and_dispatch_request;
    use crate::context::account::{Account, Addressable, Amount};
    use crate::runtime::ParsedCode;

    // Deploy a vec of smart functions from the same creator, each
    // with `amount` XTZ. Returns a vec of hashes corresponding to
    // each sf deployed
    fn deploy_smart_functions<const N: usize>(
        scripts: [&str; N],
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        creator: &impl Addressable,
        amount: Amount,
    ) -> [SmartFunctionHash; N] {
        let mut hashes = vec![];
        for i in 0..N {
            // Safety
            // Script is valid
            let hash = Account::create_smart_function(hrt, tx, creator, amount, unsafe {
                ParsedCode::new_unchecked(scripts[i].to_string())
            })
            .unwrap();
            hashes.push(hash);
        }

        hashes.try_into().unwrap()
    }

    fn setup<'a, const N: usize>(
        host: &mut tezos_smart_rollup_mock::MockHost,
        scripts: [&'a str; N],
    ) -> (
        JsHostRuntime<'static>,
        Arc<Mutex<Transaction>>,
        PublicKeyHash,
        [SmartFunctionHash; N],
    ) {
        let mut host = JsHostRuntime::new(host);
        let tx = Arc::new(Mutex::new(jstz_core::kv::Transaction::default()));
        tx.lock().begin();
        let source_address = jstz_mock::account1();
        let hashes = deploy_smart_functions(
            scripts,
            &mut host,
            &mut tx.lock(),
            &source_address,
            0,
        );
        (host, tx, source_address, hashes)
    }

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
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
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
                .get(&mut host, &mut tx.lock(), "value")
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
                    "referrer":"KT1WEAA8whopt6FqPodVErxnQysYSkTan4wS"
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
    fn request_header_has_referrer() {
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
            assert!(request_headers["referrer"] == run_address.to_string());
        })
    }

    #[test]
    fn fetch_replaces_referrer_in_request_header() {
        TOKIO.block_on(async {

        // Code
        let run = r#"export default async (req) => {
            let address = new URL(req.url).pathname.substring(1);
            let request = new Request(`jstz://${address}`, {
                headers: {
                    Referrer: req.headers.get("referrer") // Tries to forward referrer
                }
            });
            return await fetch(request)
        }"#;
        let remote =
            r#"export default async (req) => new Response(req.headers.get("referrer"))"#;

        // Setup
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
        let run_address = hashes[0].clone();
        let remote_address = hashes[1].clone();

        // Run
        let response = process_and_dispatch_request(
            JsHostRuntime::new(&mut host),
            tx.clone(),
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
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ =
                Account::add_balance(&mut host, &mut tx.lock(), &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
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
                Account::balance(&mut host, &mut tx.lock(), &run_address).unwrap()
            );
            assert_eq!(
                2_000_000,
                Account::balance(&mut host, &mut tx.lock(), &remote_address).unwrap()
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
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ =
                Account::add_balance(&mut host, &mut tx.lock(), &run_address, 10_000_000);

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
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
                Account::balance(&mut host, &mut tx.lock(), &run_address).unwrap()
            );
            assert_eq!(
                0,
                Account::balance(&mut host, &mut tx.lock(), &remote_address).unwrap()
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
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ =
                Account::add_balance(&mut host, &mut tx.lock(), &run_address, 10_000_000);

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
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
                9_000_000,
                Account::balance(&mut host, &mut tx.lock(), &run_address).unwrap()
            );
            assert_eq!(
                1_000_000,
                Account::balance(&mut host, &mut tx.lock(), &remote_address).unwrap()
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
            let (mut host, tx, _, hashes) = setup(&mut host, [run, remote]);
            let run_address = hashes[0].clone();
            let remote_address = hashes[1].clone();

            // Adds 10 XTZ
            let _ = Account::add_balance(
                &mut host,
                &mut tx.lock(),
                &remote_address,
                10_000_000,
            );

            // Run
            let response = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
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
                Account::balance(&mut host, &mut tx.lock(), &run_address).unwrap()
            );
            assert_eq!(
                10_000_000,
                Account::balance(&mut host, &mut tx.lock(), &remote_address).unwrap()
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
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::ext::jstz_kv::kv::Kv::new(remote_address.to_string());
            let mut tx = tx.lock();
            let result = kv.get(&mut host, &mut tx, "test");
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
                jstz_mock::account1().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}/{}", run_address, remote_address).as_str())
                    .unwrap(),
                vec![],
                None,
            )
            .await;

            // check transaction was commited with unawaited on values
            let kv = jstz_runtime::Kv::new(remote_address.to_string());
            let mut tx = tx.lock();
            let result = kv.get(&mut host, &mut tx, "test");
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

    // Fetch API compliance
    // TODO: https://github.com/jstz-dev/jstz/pull/982
    #[allow(dead_code)]
    fn request_get_reader_supported() {}
}
