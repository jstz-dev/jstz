use std::io::Read;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_api::http::request::Request;
use jstz_api::http::{body::HttpBody, request::RequestClass, response::Response};
use jstz_core::native::JsNativeObject;
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::{Kv, Transaction},
    runtime::{self, with_global_host},
    Module, Realm,
};
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    api,
    context::account::{Account, Address},
    Error, Result,
};

pub mod headers {

    use super::*;
    pub const REFERRER: &str = "Referer";

    pub fn test_and_set_referrer(request: &Request, referer: &Address) -> JsResult<()> {
        if request.headers().deref().contains_key(REFERRER) {
            return Err(JsError::from_native(
                JsNativeError::error().with_message("Referer already set"),
            ));
        }

        request
            .headers()
            .deref_mut()
            .set(REFERRER, &referer.to_base58())
    }
}

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
                        FunctionObjectBuilder::new(context.realm(), unsafe {
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

fn register_web_apis(realm: &Realm, context: &mut Context<'_>) {
    realm.register_api(jstz_api::ConsoleApi, context);
    realm.register_api(jstz_api::url::UrlApi, context);
    realm.register_api(jstz_api::http::HttpApi, context);
    realm.register_api(jstz_api::TextEncoderApi, context);
}

#[derive(Debug, PartialEq, Eq, Clone, Deref, DerefMut, Trace, Finalize)]
pub struct Script(Module);

impl Script {
    fn get_default_export(&self, context: &mut Context<'_>) -> JsResult<JsValue> {
        self.namespace(context).get(js_string!("default"), context)
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

    pub fn load(
        tx: &mut Transaction,
        address: &Address,
        context: &mut Context<'_>,
    ) -> Result<Self> {
        let src = with_global_host(|hrt| {
            Account::contract_code(hrt, tx, address)?.ok_or(Error::InvalidAddress)
        })?;

        with_global_host(|hrt| debug_msg!(hrt, "Evaluating: {src:?}\n"));

        Ok(Self::parse(Source::from_bytes(&src), context)?)
    }

    pub fn parse<R: Read>(
        src: Source<'_, R>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let module = Module::parse(src, Some(Realm::new(context)?), context)?;

        Ok(Self(module))
    }

    fn register_apis(&self, contract_address: Address, context: &mut Context<'_>) {
        register_web_apis(self.realm(), context);
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
    }

    /// Initialize the script, registering all associated runtime APIs
    /// and evaluating the module of the script
    pub fn init(
        &self,
        contract_address: Address,
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

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        tx: &mut Transaction,
        address: &Address,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        // 1. Load script
        let script = Script::load(tx, address, context)?;

        // 2. Evaluate the script's module
        let script_promise = script.init(address.clone(), context)?;

        // 3. Once evaluated, call the script's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context.realm(), unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, (script, request), context| script.run(request, context),
                        (script, request.clone()),
                    )
                })
                .build(),
            ),
            None,
            context,
        )?;

        Ok(result.into())
    }
}

pub mod run {

    use super::*;
    use crate::{operation, receipt};

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

    pub fn execute(
        hrt: &mut (impl HostRuntime + 'static),
        tx: &mut Transaction,
        source: &Address,
        run: operation::RunContract,
    ) -> Result<receipt::RunContract> {
        let operation::RunContract {
            uri,
            method,
            headers,
            body,
            gas_limit,
        } = run;

        // 1. Initialize runtime (with Web APIs to construct request)
        let rt = &mut jstz_core::Runtime::new(gas_limit.unwrap_or(usize::MAX))?;
        register_web_apis(&rt.realm().clone(), rt);

        // 2. Extract address from request
        let address = Address::from_base58(&uri.host().expect("Expected host"))?;

        // 3. Deserialize request
        let http_request = create_http_request(uri, method, headers, body);

        let request = JsNativeObject::new::<RequestClass>(
            Request::from_http_request(http_request, rt)?,
            rt,
        )?;

        // 4. Set referer as the source address of the operation
        headers::test_and_set_referrer(&request.deref(), source)?;

        // 5. Run :)
        let result: JsValue = runtime::with_host_runtime(hrt, || {
            jstz_core::future::block_on(async move {
                let result = Script::load_init_run(tx, &address, request.inner(), rt)?;

                rt.resolve_value(&result).await
            })
        })?;

        // 6. Serialize response
        let response = Response::try_from_js(&result)?;

        let (http_parts, body) = Response::to_http_response(&response).into_parts();

        Ok(receipt::RunContract {
            body,
            status_code: http_parts.status,
            headers: http_parts.headers,
        })
    }
}
