use std::{io::Read, ops::BitXor};

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_api::{
    http::{
        body::HttpBody,
        request::{Request, RequestClass},
        response::Response,
    },
    js_log::set_js_logger,
};
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, native::JsNativeObject, runtime,
    Module, Realm,
};
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    api::{self, TraceData},
    context::account::{Account, Address, Amount},
    operation::OperationHash,
    request_logger::{log_request_end, log_request_start},
    Error, Result,
};

use crate::js_logger::JsonLogger;

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
    f: fn(&JsValue, &mut Context<'_>) -> JsResult<()>,
    context: &mut Context<'_>,
) -> JsResult<JsValue> {
    match value.as_promise() {
        Some(promise) => {
            let promise = JsPromise::from_object(promise.clone()).unwrap();
            let result = promise.then(
                Some(
                    FunctionObjectBuilder::new(context.realm(), unsafe {
                        NativeFunction::from_closure(move |_, args, context| {
                            let value = args.get_or_undefined(0).clone();
                            let _ = f(&value, context);
                            Ok(value)
                        })
                    })
                    .build(),
                ),
                None,
                context,
            )?;
            Ok(result.into())
        }
        None => {
            f(&value, context)?;
            Ok(value)
        }
    }
}

fn compute_seed(address: &Address, operation_hash: &OperationHash) -> u64 {
    let mut seed: u64 = 0;
    for byte in operation_hash.as_array().iter().chain(address.as_bytes()) {
        seed = seed.rotate_left(8).bitxor(*byte as u64)
    }

    seed
}

pub fn register_web_apis(realm: &Realm, context: &mut Context<'_>) {
    realm.register_api(jstz_api::url::UrlApi, context);
    realm.register_api(jstz_api::urlpattern::UrlPatternApi, context);
    realm.register_api(jstz_api::http::HttpApi, context);
    realm.register_api(jstz_api::encoding::EncodingApi, context);
    realm.register_api(jstz_api::ConsoleApi, context);
    realm.register_api(jstz_api::file::FileApi, context);
}

pub fn register_jstz_apis(
    realm: &Realm,
    address: &Address,
    seed: u64,
    context: &mut Context<'_>,
) {
    realm.register_api(
        jstz_api::KvApi {
            address: address.clone(),
        },
        context,
    );
    realm.register_api(jstz_api::RandomApi { seed }, context);
    realm.register_api(
        api::LedgerApi {
            address: address.clone(),
        },
        context,
    );
    realm.register_api(
        api::SmartFunctionApi {
            address: address.clone(),
        },
        context,
    );
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
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        address: &Address,
        context: &mut Context<'_>,
    ) -> Result<Self> {
        let src =
            Account::function_code(hrt, tx, address)?.ok_or(Error::InvalidAddress)?;

        Ok(Self::parse(Source::from_bytes(&src), context)?)
    }

    pub fn parse<R: Read>(
        src: Source<'_, R>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let module = Module::parse(src, Some(Realm::new(context)?), context)?;
        Ok(Self(module))
    }

    fn register_apis(
        &self,
        address: &Address,
        operation_hash: &OperationHash,
        context: &mut Context<'_>,
    ) {
        register_web_apis(self.realm(), context);
        register_jstz_apis(
            self.realm(),
            address,
            compute_seed(address, operation_hash),
            context,
        );
    }

    /// Initialize the script, registering all associated runtime APIs
    /// and evaluating the module of the script
    pub fn init(
        &self,
        address: &Address,
        operation_hash: &OperationHash,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> {
        self.register_apis(address, operation_hash, context);

        self.realm().eval_module(self, context)
    }

    /// Deploys a script
    pub fn deploy(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        source: &Address,
        code: String,
        balance: Amount,
    ) -> Result<Address> {
        let nonce = Account::nonce(hrt, tx, source)?;

        let address = Address::digest(
            format!("{}{}{}", source, code, nonce.to_string(),).as_bytes(),
        )?;

        let account = Account::create(hrt, tx, &address, balance, Some(code));
        if account.is_ok() {
            debug_msg!(hrt, "[📜] Smart function deployed: {address}\n");
        } else if let Err(Error::InvalidAddress) = account {
            debug_msg!(hrt, "[📜] Smart function was already deployed: {address}\n");
        } else {
            // Unreachable?
            debug_msg!(hrt, "[📜] Smart function deployment failed. \n");
            account?
        }

        Ok(address)
    }

    /// Runs the script
    pub fn run(
        &self,
        address: &Address,
        operation_hash: &OperationHash,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let context = &mut self.realm().context_handle(context);

        // 1. Begin a new transaction
        runtime::with_js_tx(|tx| tx.begin());

        // 2. Initialize host defined data

        {
            host_defined!(context, mut host_defined);

            let trace_data = TraceData {
                address: address.clone(),
                operation_hash: operation_hash.clone(),
            };

            host_defined.insert(trace_data);
        }

        // 3. Set logger
        set_js_logger(&JsonLogger);
        log_request_start(address.clone(), operation_hash.to_string());

        // 4. Invoke the script's handler
        let result =
            self.invoke_handler(&JsValue::undefined(), &[request.clone()], context)?;

        // TODO: decode request and add more fields to the request (status, header etc).
        log_request_end(address.clone(), operation_hash.to_string());

        // 4. Ensure that the transaction is committed
        on_success(
            result,
            |value, _context| {
                runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<()> {
                    let response = Response::try_from_js(value)?;

                    // If status code is 2xx, commit transaction
                    if response.ok() {
                        tx.commit(hrt)?;
                    } else {
                        tx.rollback()?;
                    }

                    Ok(())
                })
            },
            context,
        )
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        address: Address,
        operation_hash: OperationHash,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        // 1. Load script

        let script = runtime::with_js_hrt_and_tx(|hrt, tx| {
            Script::load(hrt, tx, &address, context)
        })?;

        // 2. Evaluate the script's module
        let script_promise = script.init(&address, &operation_hash, context)?;

        // 3. Once evaluated, call the script's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context.realm(), unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_, _, (address, operation_hash, script, request), context| {
                            {
                                script.run(address, operation_hash, request, context)
                            }
                        },
                        (address, operation_hash, script, request.clone()),
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
    use crate::{
        operation::{self, OperationHash},
        receipt,
    };

    fn create_http_request(
        uri: http::Uri,
        method: http::Method,
        headers: http::HeaderMap,
        body: HttpBody,
    ) -> Result<http::Request<HttpBody>> {
        let mut builder = http::Request::builder().uri(uri).method(method);

        *builder.headers_mut().ok_or(Error::InvalidHttpRequest)? = headers;

        builder.body(body).map_err(|_| Error::InvalidHttpRequest)
    }

    pub fn execute(
        hrt: &mut (impl HostRuntime + 'static),
        tx: &mut Transaction,
        source: &Address,
        run: operation::RunFunction,
        operation_hash: OperationHash,
    ) -> Result<receipt::RunFunction> {
        let operation::RunFunction {
            uri,
            method,
            headers,
            body,
            gas_limit,
        } = run;

        // 1. Initialize runtime (with Web APIs to construct request)
        let rt = &mut jstz_core::Runtime::new(gas_limit)?;
        register_web_apis(&rt.realm().clone(), rt);

        // 2. Extract address from request
        let address = Address::from_base58(uri.host().ok_or(Error::InvalidAddress)?)?;

        // 3. Deserialize request
        let http_request = create_http_request(uri, method, headers, body)?;

        let request = JsNativeObject::new::<RequestClass>(
            Request::from_http_request(http_request, rt)?,
            rt,
        )?;

        // 4. Set referer as the source address of the operation
        headers::test_and_set_referrer(&request.deref(), source)?;

        // 5. Run :)
        let result: JsValue = {
            let rt = &mut *rt;
            runtime::enter_js_host_context(hrt, tx, || {
                jstz_core::future::block_on(async move {
                    let result = Script::load_init_run(
                        address,
                        operation_hash,
                        request.inner(),
                        rt,
                    )?;

                    rt.resolve_value(&result).await
                })
            })
        }
        .map_err(|err| {
            if rt.instructions_remaining() == 0 {
                Error::GasLimitExceeded
            } else {
                err.into()
            }
        })?;

        // 6. Serialize response
        let response = Response::try_from_js(&result)?;

        let (http_parts, body) = Response::to_http_response(&response).into_parts();

        Ok(receipt::RunFunction {
            body,
            status_code: http_parts.status,
            headers: http_parts.headers,
        })
    }
}

pub mod deploy {
    use super::*;
    use crate::{operation, receipt};

    pub fn execute(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        source: &Address,
        deployment: operation::DeployFunction,
    ) -> Result<receipt::DeployFunction> {
        let operation::DeployFunction {
            function_code,
            account_credit,
        } = deployment;

        let address = Script::deploy(hrt, tx, source, function_code, account_credit)?;

        Ok(receipt::DeployFunction { address })
    }
}
