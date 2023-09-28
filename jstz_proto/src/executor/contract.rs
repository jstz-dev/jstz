use std::io::Read;

use boa_engine::{
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};

use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use either::Either;
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::{Kv, Transaction},
    runtime, Module, Realm,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use tezos_smart_rollup::prelude::debug_msg;

use crate::{api, Result};

fn on_success(
    value: JsValue,
    f: fn(&mut Context<'_>),
    context: &mut Context<'_>,
) -> JsValue {
    match value.as_promise() {
        Some(promise) => {
            let promise = JsPromise::from_object(promise.clone()).unwrap();
            promise
                .then(
                    Some(
                        FunctionObjectBuilder::new(context, unsafe {
                            NativeFunction::from_closure(move |_, args, context| {
                                f(context);
                                Ok(args.get_or_undefined(0).clone())
                            })
                        })
                        .build(),
                    ),
                    None,
                    context,
                )
                .unwrap()
                .into()
        }
        None => {
            f(context);
            value
        }
    }
}

fn _finally(
    value: JsValue,
    on_finally: fn(&mut Context<'_>),
    context: &mut Context<'_>,
) -> JsValue {
    match value.as_promise() {
        Some(promise) => {
            let promise = JsPromise::from_object(promise.clone()).unwrap();
            promise
                .finally(
                    FunctionObjectBuilder::new(context, unsafe {
                        NativeFunction::from_closure(move |_, _, context| {
                            on_finally(context);
                            Ok(JsValue::undefined())
                        })
                    })
                    .build(),
                    context,
                )
                .unwrap()
                .into()
        }
        None => {
            on_finally(context);
            value
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deref, DerefMut, Trace, Finalize)]
pub struct Script(Module);

impl Script {
    fn get_default_export(&self, context: &mut Context<'_>) -> JsResult<JsValue> {
        self.namespace(context).get("default", context)
    }

    fn invoke_handler(
        &self,
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let default_export = self.get_default_export(context)?;

        let handler = default_export.as_object().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("Failed to convert `default` export to js object"),
            )
        })?;

        handler.call(this, args, context)
    }

    pub fn parse<R: Read>(
        src: Source<'_, R>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let module = Module::parse(src, Some(Realm::new(context)?), context)?;

        Ok(Self(module))
    }

    fn register_apis(&self, contract_address: PublicKeyHash, context: &mut Context<'_>) {
        self.realm().register_api(jstz_api::ConsoleApi, context);
        self.realm().register_api(
            jstz_api::KvApi {
                contract_address: contract_address.clone(),
            },
            context,
        );
        self.realm().register_api(
            api::LedgerApi {
                contract_address: contract_address.clone(),
            },
            context,
        );
        self.realm()
            .register_api(api::ContractApi { contract_address }, context);
        self.realm().register_api(jstz_api::url::UrlApi, context);
        self.realm().register_api(jstz_api::http::HttpApi, context);
        self.realm().register_api(jstz_api::TextEncoderApi, context);
    }

    /// Initialize the script, registering all associated runtime APIs
    /// and evaluating the module of the script
    pub fn init(
        &self,
        contract_address: PublicKeyHash,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> {
        self.register_apis(contract_address, context);

        self.realm().eval_module(&self, context)
    }

    /// Runs the script
    pub fn run(&self, request: &JsValue, context: &mut Context<'_>) -> JsResult<JsValue> {
        let context = &mut self.realm().context_handle(context);

        // 1. Register `Kv` and `Transaction` objects in `HostDefined`
        // FIXME: `Kv` and `Transaction` should be externally provided
        {
            host_defined!(context, mut host_defined);

            let kv = Kv::new();
            let tx = kv.begin_transaction();

            host_defined.insert(kv);
            host_defined.insert(tx);
        }

        // 2. Invoke the script's handler
        let result =
            self.invoke_handler(&JsValue::undefined(), &[request.clone()], context)?;

        // 3. Ensure that the transaction is committed
        let result = on_success(
            result,
            |context| {
                host_defined!(context, mut host_defined);

                runtime::with_global_host(|rt| {
                    let mut kv = host_defined
                        .remove::<Kv>()
                        .expect("Rust type `Kv` should be defined in `HostDefined`");

                    let tx = host_defined.remove::<Transaction>().expect(
                        "Rust type `Transaction` should be defined in `HostDefined`",
                    );

                    kv.commit_transaction(rt, *tx)
                        .expect("Failed to commit transaction");
                })
            },
            context,
        );

        Ok(result)
    }
}

pub mod run {

    use jstz_api::http::{
        body::HttpBody,
        request::{Request, RequestClass},
        response::Response,
    };
    use jstz_core::{
        native::JsNativeObject,
        runtime::{with_global_host, with_host_runtime},
    };

    use super::*;
    use crate::{context::account::Account, operation, receipt, Error};

    fn create_http_request(
        uri: http::Uri,
        method: http::Method,
        headers: http::HeaderMap,
        body: HttpBody,
    ) -> http::Request<HttpBody> {
        let mut builder = http::Request::builder().uri(uri).method(method);

        *builder.headers_mut().unwrap() = headers;

        builder.body(body).expect("Expected valid http request")
    }

    pub fn execute_js(
        tx: &mut Transaction,
        contract_address: PublicKeyHash,
        request: Either<http::Request<HttpBody>, Request>,
        referer: &PublicKeyHash,
    ) -> JsResult<JsValue> {
        // 1. Load contract code
        let contract_code = with_global_host(|hrt| -> Result<_> {
            let contract_code = Account::contract_code(hrt, tx, &contract_address)?
                .ok_or(Error::InvalidAddress)?;
            debug_msg!(hrt, "Evaluating: {contract_code:?}\n");
            Ok(contract_code)
        })?;

        // 2. Initialize runtime
        let rt = &mut jstz_core::Runtime::new()?;

        // 3. Initialize script
        let script = Script::parse(Source::from_bytes(&contract_code), rt)
            .expect("Failed to parse script");
        let script_promise = script.init(contract_address, rt)?;

        // 3. Execute
        jstz_core::future::block_on(async move {
            rt.resolve_value(&script_promise.into())
                .await
                .expect("Failed to resolve script promise");
            let js_request = {
                let context = &mut script.realm().context_handle(rt);
                let mut request = match request {
                    Either::Left(http_request) => {
                        Request::from_http_request(http_request, context)?
                    }
                    Either::Right(request) => request,
                };
                request.set_referrer(referer)?;
                JsNativeObject::new::<RequestClass>(request, context)
            }?;

            let result = script.run(js_request.inner(), rt)?;

            rt.resolve_value(&result).await
        })
    }
    pub fn execute(
        hrt: &mut (impl HostRuntime + 'static),
        tx: &mut Transaction,
        run: operation::RunContract,
    ) -> Result<receipt::RunContract> {
        let operation::RunContract {
            uri,
            method,
            headers,
            body,
            referer,
        } = run;

        if headers.contains_key("Referer") {
            return Err(Error::RefererShouldNotBeSet);
        }

        let contract_address =
            PublicKeyHash::from_base58(&uri.host().expect("Expected host"))?;
        let http_request = create_http_request(uri, method, headers, body);

        let result = with_host_runtime(hrt, || {
            execute_js(tx, contract_address, Either::Left(http_request), &referer)
        })?;

        let response = Response::try_from_js(&result)?;

        let (http_parts, body) = Response::to_http_response(&response).into_parts();

        Ok(receipt::RunContract {
            body,
            status_code: http_parts.status,
            headers: http_parts.headers,
        })
        /*
        let contract_code = Account::contract_code(hrt, tx, &contract_address)?
            .ok_or(Error::InvalidAddress)?;

        debug_msg!(hrt, "Evaluating: {contract_code:?}\n");

        let result: JsValue = runtime::with_host_runtime(hrt, || {
            // 2. Initialize script
            let script = Script::parse(Source::from_bytes(&contract_code), rt)
                .expect("Failed to parse script");

            let script_promise = script.init(contract_address, rt)?;
            let http_request = create_http_request(uri, method, headers, body);

            jstz_core::future::block_on(async move {
                rt.resolve_value(&script_promise.into())
                    .await
                    .expect("Failed to resolve script promise");

                let request = {
                    let context = &mut script.realm().context_handle(rt);
                    JsNativeObject::new::<RequestClass>(
                    Request::from_http_request(http_request, context)?,
                    context,
                )
                }?;

                let result = script.run(request.inner(), rt)?;

                rt.resolve_value(&result).await
            })
        })?;

        todo!()*/
    }
}
