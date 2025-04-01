#![allow(unused)]
use std::{borrow::Cow, cell::RefCell, future::Future, pin::Pin, rc::Rc};

use deno_core::{
    error::JsError,
    futures::{FutureExt, TryFutureExt},
    resolve_import, serde_v8,
    v8::{self, Global, Handle, HandleScope, Local},
    BufView, ByteString, JsBuffer, ModuleSpecifier, OpState, Resource, ResourceId,
    StaticModuleLoader, ToJsBuffer,
};
use deno_error::JsErrorClass;
use deno_fetch_ext::{FetchHandler, FetchReturn, ResBody};
use jstz_api::http::{
    body,
    header::{self, Header},
};
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{
    error::RuntimeError,
    sys::{
        js::{
            class::Promise,
            convert::{FromV8, Serde, ToV8},
        },
        Headers, Request, RequestInit, Response,
    },
    JstzRuntime, JstzRuntimeOptions, Protocol,
};
use serde::{Deserialize, Serialize};
use tokio::pin;
use url::Url;

use crate::{
    context::account::{Account, Address, AddressKind, Addressable},
    executor::smart_function::jstz_run,
};

pub struct JstzFetchHandler;

impl FetchHandler for JstzFetchHandler {
    type CreateHttpClientArgs = ();

    type FetchError = FetchError;

    type Options = ();

    fn fetch(
        state: &mut OpState,
        method: ByteString,
        url: String,
        headers: Vec<(ByteString, ByteString)>,
        client_rid: Option<u32>,
        has_body: bool,
        data: Option<JsBuffer>,
        resource: Option<ResourceId>,
    ) -> Result<FetchReturn, Self::FetchError> {
        fetch(state, method, url, headers, data)
    }

    async fn fetch_send(
        state: Rc<RefCell<OpState>>,
        rid: ResourceId,
    ) -> Result<deno_fetch_ext::FetchResponse, Self::FetchError> {
        fetch_send(state, rid).await
    }

    fn custom_client(
        state: &mut deno_core::OpState,
        args: Self::CreateHttpClientArgs,
    ) -> Result<ResourceId, Self::FetchError> {
        Err(FetchError::Err)
    }
}

pub struct FetchRequestResource {
    pub future: Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>>>>,
    pub url: Url,
    pub metadata: FetchMetadata,
}

pub struct FetchMetadata {
    pub from: SmartFunctionHash,
    pub to: SmartFunctionHash,
}

impl Resource for FetchRequestResource {}

pub enum ResponseBody {
    Vector(Vec<u8>),
    Buffer(JsBuffer),
}

pub struct FetchResponseResource {
    body: RefCell<Option<ResponseBody>>,
}

impl Resource for FetchResponseResource {
    fn read(self: Rc<Self>, limit: usize) -> deno_core::AsyncResult<deno_core::BufView> {
        Box::pin(async move {
            if let Some(body) = self.body.borrow_mut().take() {
                return Ok(match body {
                    ResponseBody::Buffer(body) => BufView::from(body),
                    ResponseBody::Vector(body) => BufView::from(body),
                });
            }
            Ok(BufView::empty())
        })
    }
}

pub enum SupportedScheme {
    Jstz,
}

impl TryFrom<&Url> for SupportedScheme {
    type Error = FetchError;

    fn try_from(value: &Url) -> Result<Self, Self::Error> {
        match value.scheme() {
            "tezos" => Ok(Self::Jstz),
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_string())),
        }
    }
}

fn fetch(
    state: &mut OpState,
    method: ByteString,
    url: String,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<JsBuffer>,
) -> Result<FetchReturn, FetchError> {
    let url = Url::try_from(url.as_str())?;
    let scheme = SupportedScheme::try_from(&url)?;
    let fetch_request_resource = match scheme {
        SupportedScheme::Jstz => {
            let protocol = state.borrow_mut::<Protocol>();
            let tx = &mut protocol.tx;
            let host = &mut protocol.host;
            let from = protocol.address.clone();
            let address = parse_address(tx, host, &url)?;
            let headers = process_request_headers(tx, host, headers, &from, &address)?;
            match address.kind() {
                AddressKind::User => todo!(),
                AddressKind::SmartFunction => {
                    let address = address.as_smart_function().unwrap();
                    let protocol = Protocol::new(host, tx, address.clone());
                    let fut = run_smart_function_inner(
                        protocol,
                        method,
                        url.clone(),
                        headers,
                        data,
                    );
                    FetchRequestResource {
                        future: Box::pin(fut),
                        url,
                        metadata: FetchMetadata {
                            from,
                            to: address.clone(),
                        },
                    }
                }
            }
        }
    };
    let request_rid = state.resource_table.add(fetch_request_resource);
    Ok(FetchReturn {
        request_rid,
        cancel_handle_rid: None,
    })
}

async fn run_smart_function_inner(
    mut proto: Protocol,
    method: ByteString,
    url: Url,
    headers: Vec<(ByteString, ByteString)>,
    mut data: Option<JsBuffer>,
) -> Result<FetchResponse, FetchError> {
    // 1. Load script
    let script = load_script(&mut proto.tx, &mut proto.host, &proto.address)?;

    // 2. Prepare runtime
    let specifier = resolve_import(
        "file://jstz/accounts/root",
        format!("//sf/address.js").as_str(),
    )
    .unwrap();
    let module_loader = StaticModuleLoader::with(specifier.clone(), script.to_string());
    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        module_loader: Rc::new(module_loader),
        fetch_extension: deno_fetch_ext::deno_fetch::init_ops_and_esm::<JstzFetchHandler>(
            (),
        ),
        protocol: Some(proto),
        ..Default::default()
    });

    // 3. Prepare request
    let request = {
        let scope = &mut runtime.handle_scope();
        let headers = Headers::new_with_sequence_v8(scope, headers.into());
        let request_init = RequestInit::new(scope);
        request_init.set_headers(scope, headers);
        request_init.set_method(scope, method);
        if let Some(body) = data.take() {
            let body = body.to_v8(scope);
            request_init.set_body(scope, body);
        }
        let request =
            Request::new_with_string_and_init(scope, url.to_string(), request_init);
        let request = request.to_v8(scope);
        v8::Global::new(scope, request)
    };

    // 4. Run
    let args = [request];
    let id = runtime.execute_main_module(&specifier).await?;
    let result = runtime.call_default_handler(id, &args).await?;
    let response = convert_js_to_http_response(&mut runtime, result).await?;
    Ok(response)
}

async fn fetch_send(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
) -> Result<deno_fetch_ext::FetchResponse, FetchError> {
    let request = state
        .borrow_mut()
        .resource_table
        .take::<FetchRequestResource>(rid)
        .unwrap();

    let request = Rc::try_unwrap(request)
        .ok()
        .expect("multiple op_fetch_send ongoing");

    match request.future.await {
        Ok(resp) => {
            let mut state = state.borrow_mut();
            let headers = {
                let proto = state.borrow_mut::<Protocol>();
                process_response_headers(
                    proto.tx,
                    &mut proto.host,
                    resp.headers,
                    &request.metadata.from,
                    &request.metadata.to,
                )?
            };
            let response_rid = state.resource_table.add(FetchResponseResource {
                body: RefCell::new(resp.body.map(ResponseBody::Buffer)),
            });

            let response = deno_fetch_ext::FetchResponse {
                status: resp.status,
                status_text: resp.status_text,
                headers,
                url: request.url.to_string(),
                response_rid,
                content_length: None,
                remote_addr_ip: None,
                remote_addr_port: None,
                error: None,
            };
            Ok(response)
        }
        Err(err) => {
            let error_body: FetchErrorBody = err.into();
            let mut state = state.borrow_mut();
            let error = match serde_json::to_vec(&error_body) {
                Ok(body) => Some(ResponseBody::Vector(body)),
                Err(_) => None,
            };
            let response_rid = state.resource_table.add(FetchResponseResource {
                body: RefCell::new(error),
            });
            let response = deno_fetch_ext::FetchResponse {
                status: 600,
                status_text: "RuntimeError".to_string(),
                headers: vec![],
                url: request.url.to_string(),
                response_rid,
                content_length: None,
                remote_addr_ip: None,
                remote_addr_port: None,
                error: None,
            };
            Ok(response)
        }
    }
}

fn parse_address(
    tx: &mut Transaction,
    host: &mut impl HostRuntime,
    url: &Url,
) -> Result<Address, FetchError> {
    let raw_address = url.host().ok_or(url::ParseError::EmptyHost)?;
    Address::from_base58(raw_address.to_string().as_str()).map_err(|_| FetchError::Err)
}

fn process_response_headers(
    _tx: &mut Transaction,
    _host: &mut impl HostRuntime,
    mut headers: Vec<(ByteString, ByteString)>,
    from: &impl Addressable,
    to: &impl Addressable,
) -> Result<Vec<(ByteString, ByteString)>, FetchError> {
    Ok(headers)
}

fn process_request_headers(
    _tx: &mut Transaction,
    _host: &mut impl HostRuntime,
    mut headers: Vec<(ByteString, ByteString)>,
    from: &impl Addressable,
    to: &impl Addressable,
) -> Result<Vec<(ByteString, ByteString)>, FetchError> {
    for (key, value) in &headers {
        if key.to_ascii_lowercase() == ByteString::from("referrer").to_ascii_lowercase() {
            return Err(FetchError::Err);
        }
    }
    headers.push(("Referrer".into(), from.to_base58().into()));
    Ok(headers)
}

async fn convert_js_to_http_response(
    runtime: &mut JstzRuntime,
    value: v8::Global<v8::Value>,
) -> Result<FetchResponse, FetchError> {
    let (mut response, body) = {
        let scope = &mut runtime.handle_scope();
        let local_value = v8::Local::new(scope, value);
        let response = Response::from_v8(scope, local_value);
        let headers: Vec<(ByteString, ByteString)> = {
            let mut iter = response.headers(scope).entries(scope);
            let mut buf = Vec::new();
            while let Some(item) = iter.next(scope) {
                buf.push(item);
            }
            buf
        };
        let status = response.status(scope);
        let status_text = response.status_text(scope);
        let body = response.array_buffer(scope);
        let fetch_response = FetchResponse {
            status,
            status_text,
            headers,
            body: None,
        };
        (fetch_response, body)
    };

    let body = body.with_runtime(runtime).await;
    response.body = body;
    Ok(response)
}

/// Response returned from a fetch or Smart Function run
pub struct FetchResponse {
    status: u16,
    status_text: String,
    headers: Vec<(ByteString, ByteString)>,
    body: Option<JsBuffer>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FetchError {
    #[class(type)]
    #[error("Fetch error occurred")]
    Err,
    #[class(type)]
    #[error("Unsupport scheme '{0}'")]
    UnsupportedScheme(String),
    #[class(type)]
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[class(type)]
    #[error(transparent)]
    RuntimeError(#[from] RuntimeError),
}

impl From<FetchError> for FetchErrorBody {
    fn from(value: FetchError) -> Self {
        Self {
            class: value.get_class(),
            message: Some(value.get_message()),
        }
    }
}

#[derive(Serialize)]
pub struct FetchErrorBody {
    class: Cow<'static, str>,
    message: Option<Cow<'static, str>>,
}

fn load_script<'a>(
    tx: &'a mut Transaction,
    hrt: &impl HostRuntime,
    address: &SmartFunctionHash,
) -> Result<&'a str, FetchError> {
    Account::function_code(hrt, tx, address).map_err(|_| FetchError::Err)
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use deno_core::{resolve_import, PollEventLoopOptions, StaticModuleLoader};
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
    use jstz_runtime::*;

    use crate::context::account::{Account, Address, ParsedCode};

    use super::JstzFetchHandler;

    #[tokio::test]
    async fn test_fetch() {
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let address =
            SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                .unwrap();
        let mut tx = jstz_core::kv::Transaction::default();
        tx.begin();
        let protocol = Some(Protocol::new(&mut host, &mut tx, address.clone()));
        let source = Address::User(jstz_mock::account1());
        let fetched_script = r#"
            const handler = async (req) => {
                let reqBody = await req.json();
                return new Response(JSON.stringify(reqBody));
            }
            export default handler;
        "#;
        Account::add_balance(&mut host, &mut tx, &source, 10000).expect("add balance");
        let func_addr = Account::create_smart_function(
            &mut host,
            &mut tx,
            &source,
            100,
            ParsedCode(fetched_script.to_string()),
        )
        .unwrap();

        let code = format!(
            r#"
            const call = async () => {{
                let request = new Request("tezos://{func_addr}", {{
                    method: "POST",
                    body: JSON.stringify({{
                        message: "request Hello world!",
                    }}),
                    headers: {{
                        "content-type": "application/json",
                    }}
                }})
                let response = await fetch(request);
                let body = await response.json();
                console.log(body)
                console.log(response.statusText)
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
            fetch_extension: deno_fetch_ext::deno_fetch::init_ops_and_esm::<
                JstzFetchHandler,
            >(()),
            module_loader: Rc::new(module_loader),
            ..Default::default()
        });
        let id = runtime.execute_main_module(&specifier).await.unwrap();
        let result = runtime.call_default_handler(id, &[]).await.unwrap();
    }
}
