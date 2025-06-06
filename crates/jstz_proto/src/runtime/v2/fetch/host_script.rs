use crate::runtime::v2::fetch::error::{FetchError, Result};
use crate::runtime::v2::fetch::http::{Body, Response};

use deno_core::{resolve_import, v8, ByteString, StaticModuleLoader};
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_runtime::sys::{
    FromV8, Headers as JsHeaders, Request as JsRequest, RequestInit as JsRequestInit,
    Response as JsResponse, ToV8,
};
use jstz_runtime::JstzRuntime;
use jstz_runtime::{JstzRuntimeOptions, ProtocolContext};
use std::rc::Rc;
use url::Url;

use crate::context::account::{Account, Address};
use crate::runtime::v2::fetch::fetch_handler::ProtoFetchHandler;

pub struct HostScript;

impl HostScript {
    pub async fn route(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        from: Address,
        method: ByteString,
        url: Url,
        headers: Vec<(ByteString, ByteString)>,
        body: Option<Body>,
    ) -> Result<Response> {
        let path = url.path();
        if path.starts_with("/balances") {
            return Self::handle_balance(host, tx, from, method, url, headers, body)
                .await;
        }

        todo!()
    }

    // - Loads the smart function script at `address`
    // - Bootstraps a new runtime with new context and module loader
    // - Runs the smart function
    pub async fn load_and_run(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: SmartFunctionHash,
        method: ByteString,
        url: Url,
        headers: Vec<(ByteString, ByteString)>,
        body: Option<Body>,
    ) -> Result<Response> {
        let mut body = body;

        // 0. Prepare Protocol
        let mut proto = ProtocolContext::new(host, tx, address.clone());

        // 1. Load script
        let script = { Self::load_script(tx, &mut proto.host, &proto.address)? };

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
            let request = JsRequest::new_with_string_and_init(
                scope,
                url.to_string(),
                request_init,
            )?;
            let request = request.to_v8(scope)?;
            v8::Global::new(scope, request)
        };

        // 4. Run
        let args = [request];
        let id = runtime.execute_main_module(&specifier).await?;
        let result = runtime.call_default_handler(id, &args).await?;
        let response = Self::convert_js_to_response(&mut runtime, result)
            .await
            .map_err(|_| FetchError::InvalidResponseType)?;
        Ok(response)
    }

    fn load_script(
        tx: &mut Transaction,
        host: &impl HostRuntime,
        address: &SmartFunctionHash,
    ) -> Result<String> {
        Account::function_code(host, tx, address)
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

    pub async fn handle_balance(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        self_address: Address,
        method: ByteString,
        url: Url,
        _headers: Vec<(ByteString, ByteString)>,
        _body: Option<Body>,
    ) -> Result<Response> {
        if method != "GET".into() {
            return Ok(Response {
                status: 405,
                status_text: "Method Not Allowed".to_string(),
                headers: vec![],
                body: Body::Vector("Only GET method is allowed".as_bytes().to_vec()),
            });
        }

        let path = url.path();
        let address_str = path
            .strip_prefix("/balances/")
            .ok_or_else(|| FetchError::JstzError("Invalid path format".to_string()))?;

        match Self::get_balance(host, tx, address_str, &self_address) {
            Ok(balance) => Ok(Response {
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![],
                body: Body::Vector(balance.to_string().as_bytes().to_vec()),
            }),
            Err(e) => Ok(Response {
                status: 400,
                status_text: "Bad Request".to_string(),
                headers: vec![],
                body: Body::Vector(e.to_string().as_bytes().to_vec()),
            }),
        }
    }

    fn get_balance(
        host: &mut impl HostRuntime,
        tx: &mut Transaction,
        address_str: &str,
        self_address: &Address,
    ) -> Result<u64> {
        let target_address = if address_str == "self" {
            self_address.clone().into()
        } else {
            crate::context::account::Address::from_base58(address_str)
                .map_err(|e| FetchError::JstzError(e.to_string()))?
        };

        Account::balance(host, tx, &target_address)
            .map_err(|e| FetchError::JstzError(e.to_string()))
    }
}
