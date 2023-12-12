use boa_engine::{
    js_string,
    object::{builtins::JsPromise, FunctionObjectBuilder},
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{Deref, DerefMut};
use jstz_api::http::{body::HttpBody, request::RequestClass, response::Response};
use jstz_api::{http::request::Request, js_log::set_js_logger};
use jstz_core::native::JsNativeObject;
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::Transaction,
    runtime::{self, with_global_host},
    Module, Realm,
};
use std::io::Read;
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

fn register_web_apis(realm: &Realm, context: &mut Context<'_>) {
    realm.register_api(jstz_api::url::UrlApi, context);
    realm.register_api(jstz_api::urlpattern::UrlPatternApi, context);
    realm.register_api(jstz_api::http::HttpApi, context);
    realm.register_api(jstz_api::encoding::EncodingApi, context);
    realm.register_api(jstz_api::ConsoleApi, context);
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
        contract_address: &Address,
        context: &mut Context<'_>,
    ) -> Result<Self> {
        let src = with_global_host(|hrt| {
            Account::contract_code(hrt, tx, contract_address)?
                .ok_or(Error::InvalidAddress)
        })?;

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
        contract_address: Address,
        operation_hash: &OperationHash,
        context: &mut Context<'_>,
    ) {
        register_web_apis(self.realm(), context);

        self.realm().register_api(
            jstz_api::KvApi {
                contract_address: contract_address.clone(),
            },
            context,
        );
        self.realm().register_api(
            jstz_api::RandomApi {
                contract_address: contract_address.clone(),
                operation_hash: operation_hash.clone(),
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
        operation_hash: &OperationHash,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> {
        self.register_apis(contract_address, operation_hash, context);

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
        contract_address: &Address,
        operation_hash: &OperationHash,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let context = &mut self.realm().context_handle(context);

        // 1. Register `Transaction` object in `HostDefined`
        // FIXME: `Kv` and `Transaction` should be externally provided
        {
            host_defined!(context, mut host_defined);

            let tx = Transaction::new();
            let trace_data = TraceData {
                contract_address: contract_address.clone(),
                operation_hash: operation_hash.clone(),
            };

            host_defined.insert(tx);
            host_defined.insert(trace_data);
        }
        set_js_logger(&JsonLogger);

        // 2. Set logger
        set_js_logger(&JsonLogger);
        log_request_start(contract_address.clone(), operation_hash.to_string());

        // 3. Invoke the script's handler
        let result =
            self.invoke_handler(&JsValue::undefined(), &[request.clone()], context)?;

        // TODO: decode request and add more fields to the request (status, header etc).
        log_request_end(contract_address.clone(), operation_hash.to_string());

        // 4. Ensure that the transaction is committed
        on_success(
            result,
            |value, context| {
                host_defined!(context, mut host_defined);

                runtime::with_global_host(|rt| {
                    let mut tx = host_defined.remove::<Transaction>().expect(
                        "Rust type `Transaction` should be defined in `HostDefined`",
                    );

                    let response = Response::try_from_js(value)?;

                    // If status code is 2xx, commit transaction
                    if response.ok() {
                        tx.commit::<Account>(rt)
                            .expect("Failed to commit transaction");
                    } else {
                        tx.rollback();
                    }
                    Ok(())
                })
            },
            context,
        )
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        tx: &mut Transaction,
        contract_address: Address,
        operation_hash: OperationHash,
        request: &JsValue,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        // 1. Load script
        let script = Script::load(tx, &contract_address, context)?;

        // 2. Evaluate the script's module
        let script_promise =
            script.init(contract_address.clone(), &operation_hash, context)?;

        // 3. Once evaluated, call the script's handler
        let result = script_promise.then(
            Some(
                FunctionObjectBuilder::new(context.realm(), unsafe {
                    NativeFunction::from_closure_with_captures(
                        |_,
                         _,
                         (contract_address, operation_hash, script, request),
                         context| {
                            script.run(contract_address, operation_hash, request, context)
                        },
                        (contract_address, operation_hash, script, request.clone()),
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
        run: operation::RunContract,
        operation_hash: OperationHash,
    ) -> Result<receipt::RunContract> {
        let operation::RunContract {
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
            runtime::with_host_runtime(hrt, || {
                jstz_core::future::block_on(async move {
                    let result = Script::load_init_run(
                        tx,
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

        Ok(receipt::RunContract {
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
        deployment: operation::DeployContract,
    ) -> Result<receipt::DeployContract> {
        let operation::DeployContract {
            contract_code,
            contract_credit,
        } = deployment;

        let address = Script::deploy(hrt, tx, source, contract_code, contract_credit)?;

        Ok(receipt::DeployContract {
            contract_address: address,
        })
    }
}
