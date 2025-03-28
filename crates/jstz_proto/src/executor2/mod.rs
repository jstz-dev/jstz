#![allow(unused)]
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};

use deno_core::{
    futures::TryFutureExt,
    v8::{Global, Handle, Local},
    *,
};
use deno_fetch_ext::{FetchHandler, FetchReturn, ResBody};
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, Protocol};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{context::account::Account, executor::smart_function::jstz_run};

pub struct JstzFetchHandler;

// struct RequestConstructor(v8::Global<v8::Function>);

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
        // 1. Load code
        let protocol = state.borrow_mut::<Protocol>();
        let raw_url = url.clone();
        let url = Url::try_from(url.as_str()).map_err(|_| FetchError::Err)?;
        if url.scheme() != "tezos" {
            return Err(FetchError::Err);
        }

        let raw_address = url.host().ok_or(FetchError::Err)?;
        let address = SmartFunctionHash::from_base58(raw_address.to_string().as_str())
            .map_err(|_| FetchError::Err)?;
        let code = load_script(&mut protocol.tx, &protocol.host, &address)?;

        // 2. Prepare runtime
        let specifier =
            resolve_import("file://jstz/accounts/root", "//sf/main.js").unwrap();
        let module_loader = StaticModuleLoader::with(specifier.clone(), code.to_string());
        let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
            module_loader: Rc::new(module_loader),
            ..Default::default()
        });

        // 3. Prepare input and Run
        let fut = run(specifier, runtime, raw_url, method, headers, data);

        let request_rid = state.resource_table.add(FetchRequestResource {
            future: Box::pin(fut),
            url,
        });

        Ok(FetchReturn {
            request_rid,
            cancel_handle_rid: None,
        })
    }

    async fn fetch_send(
        state: Rc<RefCell<OpState>>,
        rid: ResourceId,
    ) -> Result<deno_fetch_ext::FetchResponse, Self::FetchError> {
        let request = state
            .borrow_mut()
            .resource_table
            .take::<FetchRequestResource>(rid)
            .unwrap();

        let request = Rc::try_unwrap(request)
            .ok()
            .expect("multiple op_fetch_send ongoing");

        let resp = request.future.await.unwrap();

        let response = deno_fetch_ext::FetchResponse {
            status: resp.status,
            status_text: resp.status_text,
            headers: resp.headers,
            url: resp.url,
            response_rid: resp.response_rid,
            content_length: resp.content_length,
            remote_addr_ip: resp.remote_addr_ip,
            remote_addr_port: resp.remote_addr_port,
            error: resp.error,
        };

        Ok(response)
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
}

impl Resource for FetchRequestResource {}

async fn run(
    specifier: ModuleSpecifier,
    mut runtime: JstzRuntime,
    url: String,
    method: ByteString,
    headers: Vec<(ByteString, ByteString)>,
    data: Option<JsBuffer>,
) -> Result<FetchResponse, FetchError> {
    let state = runtime.op_state();
    let borrowed_state = state.borrow();
    let args = {
        let scope = &mut runtime.handle_scope();
        let url = serde_v8::to_v8(scope, url).unwrap();
        let url = v8::Global::new(scope, url);
        let mut request_init = v8::Object::new(scope);

        let method_key = serde_v8::to_v8(scope, "method").unwrap();
        let method_value = serde_v8::to_v8(scope, method).unwrap();
        request_init.set(scope, method_key, method_value);

        let headers_key = serde_v8::to_v8(scope, "headers").unwrap();
        let headers_value = serde_v8::to_v8(scope, headers).unwrap();

        request_init.set(scope, method_key, method_value);

        if let Some(data_inner) = data {
            let body_key = serde_v8::to_v8(scope, "body").unwrap();
            let body_value = serde_v8::to_v8(scope, data_inner).unwrap();
            request_init.set(scope, body_key, body_value);
        }

        let request_init: v8::Local<v8::Value> = request_init.into();
        let request_init = v8::Global::new(scope, request_init);
        [url, request_init]
    };

    let request_constructor = {
        let global_value = runtime
            .execute_script("<request constructor>", "Request")
            .unwrap();
        let scope = &mut runtime.handle_scope();
        let local_fn: v8::Local<v8::Function> =
            Local::new(scope, global_value).try_cast().unwrap();
        v8::Global::new(scope, local_fn)
    };

    let request = {
        let call = runtime.call_with_args(&request_constructor, &args);
        runtime
            .with_event_loop_promise(call, PollEventLoopOptions::default())
            .await
            .unwrap()
    };
    let args = [request];
    let id = runtime.execute_main_module(&specifier).await.unwrap();
    let response = runtime.call_default_handler(id, &args).await.unwrap();
    let response: FetchResponse = {
        let scope = &mut runtime.handle_scope();
        let local_response = v8::Local::new(scope, response);
        serde_v8::from_v8(scope, local_response).unwrap()
    };

    Ok(response)
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(ByteString, ByteString)>,
    pub url: String,
    pub response_rid: ResourceId,
    pub content_length: Option<u64>,
    pub remote_addr_ip: Option<String>,
    pub remote_addr_port: Option<u16>,
    /// This field is populated if some error occurred which needs to be
    /// reconstructed in the JS side to set the error _cause_.
    /// In the tuple, the first element is an error message and the second one is
    /// an error cause.
    pub error: Option<(String, String)>,
}

#[derive(Debug, ::thiserror::Error, deno_error::JsError)]
pub enum FetchError {
    #[class(type)]
    #[error("error")]
    Err,
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
    use jstz_runtime::*;

    use crate::context::account::{Account, Address, ParsedCode};

    use super::JstzFetchHandler;

    #[test]
    fn test_fetch() {
        let mut sink: Box<Vec<u8>> = Box::default();
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        host.set_debug_handler(unsafe {
            std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                sink.as_mut(),
            )
        });
        let address =
            <jstz_crypto::smart_function_hash::SmartFunctionHash as jstz_crypto::hash::Hash>::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                .unwrap();
        let mut tx = jstz_core::kv::Transaction::default();
        tx.begin();
        let protocol = Some(Protocol::new(&mut host, &mut tx, address.clone()));

        let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
            protocol,
            extensions: vec![deno_fetch_ext::deno_fetch::init_ops_and_esm::<
                JstzFetchHandler,
            >(())],
            ..Default::default()
        });

        let source = Address::User(jstz_mock::account1());
        let fetched_script = r#"
            const handler = () => new Response();
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
            let request = new Request("tezos://${func_addr}")
            console.log(JSON.stringify(request))
            let response = await fetch(request)
            console.log(JSON.stringify(response))
        "#
        );

        runtime.execute(code.as_str()).unwrap();

        println!("{}", String::from_utf8_lossy(sink.as_slice()))
    }
}
