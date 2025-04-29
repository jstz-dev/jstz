use std::{num::NonZeroU64, ops::BitXor, sync::Arc};

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, ErasedObject, FunctionObjectBuilder},
    parser::source::ReadChar,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use derive_more::{Deref, DerefMut};
use http::Uri;
use jstz_api::{
    http::{
        body::{Body, BodyWithType, HttpBody},
        header::Headers,
        request::{Request, RequestClass},
        response::{Response, ResponseClass, ResponseOptions},
    },
    js_log::set_js_logger,
};
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, native::JsNativeObject, runtime,
    Module, Realm,
};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use parking_lot::FairMutex as Mutex;
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    api::{self, TraceData},
    context::account::{Account, Addressable, Amount, ParsedCode},
    js_logger::JsonLogger,
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
    request_logger::{log_request_end, log_request_start},
    Error, Result,
};

pub mod headers {

    use crate::context::account::Addressable;

    use super::*;
    pub const REFERRER: &str = "Referer";

    pub fn test_and_set_referrer(
        request: &Request,
        referrer: &impl Addressable,
    ) -> JsResult<()> {
        if request.headers().deref().contains_key(REFERRER) {
            return Err(JsError::from_native(
                JsNativeError::error().with_message("Referer already set"),
            ));
        }

        request
            .headers()
            .deref_mut()
            .set(REFERRER, &referrer.to_base58())
    }
}

// Applies on_fullfilled or on_rejected based on either an error was raised or not.
// If the value is a promise, then we apply the on_fulfilled and on_rejected to the promise.
pub fn try_apply_to_value_or_promise<F1, F2>(
    value_or_promise: JsResult<JsValue>,
    on_fulfilled: F1,
    on_rejected: F2,
    context: &mut Context,
) -> JsResult<JsValue>
where
    F1: Fn(&JsValue, &mut Context) -> JsResult<()> + 'static,
    F2: Fn(&mut Context) -> JsResult<()> + 'static,
{
    match value_or_promise {
        Ok(value) => match value.as_promise() {
            Some(promise) => {
                let result = promise.then(
                    Some(
                        FunctionObjectBuilder::new(context.realm(), unsafe {
                            NativeFunction::from_closure(
                                move |_, args, context| -> JsResult<JsValue> {
                                    let value = args.get_or_undefined(0).clone();
                                    on_fulfilled(&value, context)?;
                                    Ok(value)
                                },
                            )
                        })
                        .build(),
                    ),
                    Some(
                        FunctionObjectBuilder::new(context.realm(), unsafe {
                            NativeFunction::from_closure(
                                move |_, args, context| -> JsResult<JsValue> {
                                    let reason = JsError::from_opaque(
                                        args.get_or_undefined(0).clone(),
                                    );
                                    on_rejected(context)?;
                                    Err(reason)
                                },
                            )
                        })
                        .build(),
                    ),
                    context,
                );
                Ok(result.into())
            }
            None => {
                on_fulfilled(&value, context)?;
                Ok(value)
            }
        },
        Err(err) => {
            on_rejected(context)?;
            Err(err)
        }
    }
}

fn compute_seed(address: &SmartFunctionHash, operation_hash: &OperationHash) -> u64 {
    let mut seed: u64 = 0;
    for byte in operation_hash.as_array().iter().chain(address.as_bytes()) {
        seed = seed.rotate_left(8).bitxor(*byte as u64)
    }

    seed
}

fn register_http_api(realm: &Realm, context: &mut Context) {
    realm.register_api(jstz_api::http::HttpApi, context);
}

pub fn register_web_apis(realm: &Realm, context: &mut Context) {
    realm.register_api(jstz_api::url::UrlApi, context);
    realm.register_api(jstz_api::urlpattern::UrlPatternApi, context);
    realm.register_api(jstz_api::http::HttpApi, context);
    realm.register_api(jstz_api::encoding::EncodingApi, context);
    realm.register_api(jstz_api::ConsoleApi, context);
    realm.register_api(jstz_api::file::FileApi, context);
}
pub fn register_jstz_apis(
    realm: &Realm,
    address: &SmartFunctionHash,
    seed: u64,
    context: &mut Context,
) {
    realm.register_api(
        api::KvApi {
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
    fn get_default_export(&self, context: &mut Context) -> JsResult<JsValue> {
        self.namespace(context).get(js_string!("default"), context)
    }

    fn invoke_handler(
        &self,
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
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
        address: &SmartFunctionHash,
        context: &mut Context,
    ) -> Result<Self> {
        let src = Account::function_code(hrt, tx, address)?;

        Ok(Self::parse(Source::from_bytes(src), context)?)
    }

    pub fn parse<R: ReadChar>(
        src: Source<'_, R>,
        context: &mut Context,
    ) -> JsResult<Self> {
        let module = Module::parse(src, Some(Realm::new(context)?), context)?;
        Ok(Self(module))
    }

    fn register_apis(
        &self,
        address: &SmartFunctionHash,
        operation_hash: &OperationHash,
        context: &mut Context,
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
        address: &SmartFunctionHash,
        operation_hash: &OperationHash,
        context: &mut Context,
    ) -> JsPromise {
        self.register_apis(address, operation_hash, context);

        self.realm().eval_module(self, context)
    }

    /// Deploys a script
    pub fn deploy(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &impl Addressable,
        code: ParsedCode,
        balance: Amount,
    ) -> Result<SmartFunctionHash> {
        // SAFETY: Smart function creation and sub_balance must be atomic
        tx.begin();
        let address = Account::create_smart_function(hrt, tx, source, balance, code)
            .and_then(|address| {
                Account::sub_balance(hrt, tx, source, balance)?;
                Ok(address)
            });

        match address {
            Ok(address) => {
                tx.commit(hrt)?;
                debug_msg!(hrt, "[📜] Smart function deployed: {}\n", address);
                Ok(address)
            }
            Err(err @ Error::AccountExists) => {
                tx.rollback()?;
                debug_msg!(hrt, "[📜] Smart function was already deployed\n");
                Err(err)
            }
            Err(err) => {
                tx.rollback()?;
                debug_msg!(hrt, "[📜] Smart function deployment failed. \n");
                Err(err)
            }
        }
    }

    /// Runs the script
    pub fn run(
        &self,
        address: &SmartFunctionHash,
        operation_hash: &OperationHash,
        request: &JsValue,
        context: &mut Context,
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
            self.invoke_handler(&JsValue::undefined(), &[request.clone()], context);

        // TODO: decode request and add more fields to the request (status, header etc).
        log_request_end(address.clone(), operation_hash.to_string());

        // 4. Ensure that the transaction is committed
        try_apply_to_value_or_promise(
            result,
            |value, _context| {
                runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<()> {
                    match Response::try_from_js(value) {
                        // commit if the value returned is a response with a 2xx status code
                        Ok(response) if response.ok() => {
                            tx.commit(hrt)?;
                        }
                        _ => tx.rollback()?,
                    };

                    Ok(())
                })
            },
            |_context| Ok(runtime::with_js_tx(|tx| tx.rollback())?),
            context,
        )
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        address: SmartFunctionHash,
        operation_hash: OperationHash,
        request: &JsValue,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. Load script
        let script = runtime::with_js_hrt_and_tx(|hrt, tx| {
            Script::load(hrt, tx, &address, context)
        })?;

        // 2. Evaluate the script's module
        let script_promise = script.init(&address, &operation_hash, context);

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
        );

        Ok(result.into())
    }
}

pub const X_JSTZ_TRANSFER: &str = "X-JSTZ-TRANSFER";
pub const X_JSTZ_AMOUNT: &str = "X-JSTZ-AMOUNT";

pub struct HostScript;

impl HostScript {
    fn create_run_function_from_request(
        request_deref: &mut GcRefMut<'_, ErasedObject, Request>,
        gas_limit: usize,
    ) -> JsResult<RunFunction> {
        let method = request_deref.method().clone();
        let uri =
            Uri::try_from(request_deref.url().clone().to_string()).map_err(|_| {
                JsError::from_native(JsNativeError::error().with_message("Invalid host"))
            })?;
        let body = request_deref.body().clone().to_http_body();
        let headers = request_deref.headers().deref_mut().to_http_headers();
        Ok(RunFunction {
            uri,
            method,
            body,
            headers,
            gas_limit,
        })
    }

    fn create_response_from_run_receipt(
        run_receipt: RunFunctionReceipt,
        context: &mut Context,
    ) -> JsResult<Response> {
        let body = Body::from_http_body(run_receipt.body, context)?;
        let options = ResponseOptions::from(run_receipt.status_code, run_receipt.headers);
        Response::new(
            BodyWithType {
                body,
                content_type: None,
            },
            options,
            context,
        )
    }

    pub fn run(
        self_address: &SmartFunctionHash,
        request: &mut GcRefMut<'_, ErasedObject, Request>,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let run = Self::create_run_function_from_request(request, 1)?;
        let response = runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<Response> {
            // 1. Begin a new transaction
            tx.begin();
            // 2. Execute jstz host smart function
            let result = jstz_run::execute_without_ticketer(hrt, tx, self_address, run);

            // 3. Commit or rollback the transaction
            match result {
                Ok(run_receipt) => {
                    if run_receipt.status_code.is_success() {
                        tx.commit(hrt)?;
                    } else {
                        tx.rollback()?;
                    }
                    Self::create_response_from_run_receipt(run_receipt, context)
                }
                Err(err) => {
                    tx.rollback()?;
                    Err(err.into())
                }
            }
        })?;

        let js_response = JsNativeObject::new::<ResponseClass>(response, context)?;
        Ok(js_response.inner().clone())
    }

    /// Extracts the XTZ transfer amount from the request headers.
    /// Returns None if the header is not present or Some(amount) if a valid amount is found.
    pub fn extract_transfer_amount(headers: &Headers) -> JsResult<Option<NonZeroU64>> {
        let header = headers.get(X_JSTZ_TRANSFER)?;

        if header.headers.is_empty() {
            return Ok(None);
        }

        if header.headers.len() > 1 {
            return Err(JsError::from_native(JsNativeError::typ().with_message(
                "Invalid transfer header: expected exactly one value",
            )));
        }

        let amount = header.headers[0]
            .parse::<NonZeroU64>()
            .map(Some)
            .map_err(|e| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message(format!("Invalid transfer amount: {}", e)),
                )
            })?;

        Ok(amount)
    }

    fn verify_headers(headers: &Headers) -> JsResult<()> {
        if headers.contains_key(X_JSTZ_AMOUNT) {
            return Err(JsError::from_native(
                JsNativeError::error()
                    .with_message("X-JSTZ-AMOUNT header should not be present"),
            ));
        }
        Ok(())
    }

    /// Transfer xtz from `src` to `dst` if the `X_JSTZ_TRANSFER` header is present & amount > 0
    /// On success, `X_JSTZ_TRANSFER` is set to `X_JSTZ_AMOUNT`
    /// Rejects if `X_JSTZ_AMOUNT` is already present in the headers or transfer failed
    pub fn handle_transfer(
        headers: &mut Headers,
        src: &impl Addressable,
        dst: &impl Addressable,
    ) -> JsResult<Option<NonZeroU64>> {
        Self::verify_headers(headers)?;
        let amt = match Self::extract_transfer_amount(headers)? {
            Some(a) => a,
            None => return Ok(None),
        };
        runtime::with_js_hrt_and_tx(|hrt, tx| {
            Account::transfer(hrt, tx, src, dst, amt.into())
                .and_then(|_| {
                    headers.remove(X_JSTZ_TRANSFER)?;
                    headers.append(X_JSTZ_AMOUNT, &amt.to_string())?;
                    Ok(())
                })
                .map_err(|e| {
                    JsError::from_native(
                        JsNativeError::eval()
                            .with_message(format!("Transfer failed: {}", e)),
                    )
                })
        })?;
        Ok(Some(amt))
    }
}

pub mod run {

    use jstz_core::Runtime;

    use super::*;
    use crate::{
        context::account::Address,
        operation::{self, OperationHash},
        receipt::RunFunctionReceipt,
    };

    /// skips the execution of the smart function for noop requests
    pub const NOOP_PATH: &str = "/-/noop";

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

    /// Handles the refund from the smart function if the response is successful.
    /// Returns the receipt of the response.
    fn handle_refund(
        response: &JsValue,
        from: &SmartFunctionHash,
        to: &impl Addressable,
    ) -> Result<RunFunctionReceipt> {
        let response = Response::try_from_js(response)?;
        if response.ok() {
            HostScript::handle_transfer(&mut response.headers().deref_mut(), from, to)?;
        }
        let (http_parts, body) = Response::to_http_response(&response).into_parts();
        Ok(RunFunctionReceipt {
            body,
            status_code: http_parts.status,
            headers: http_parts.headers,
        })
    }

    fn register_apis(rt: &mut Runtime, address: &Address) {
        match address {
            Address::SmartFunction(_) => {
                register_web_apis(&rt.realm().clone(), rt);
            }
            Address::User(_) => {
                register_http_api(&rt.realm().clone(), rt);
            }
        }
    }

    fn execute_smart_function(
        hrt: &mut impl HostRuntime,
        tx: Arc<Mutex<Transaction>>,
        rt: &mut Runtime,
        source: &impl Addressable,
        sf_address: SmartFunctionHash,
        request: JsNativeObject<Request>,
        operation_hash: OperationHash,
    ) -> Result<JsValue> {
        // Set referrer as the source address of the operation
        headers::test_and_set_referrer(&request.deref(), source)?;
        // Run :)
        {
            let rt = &mut *rt;
            runtime::enter_js_host_context(hrt, tx.clone(), || {
                jstz_core::future::block_on(async move {
                    let result = Script::load_init_run(
                        sf_address,
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
        })
    }

    pub fn execute(
        hrt: &mut impl HostRuntime,
        tx: Arc<Mutex<Transaction>>,
        source: &impl Addressable,
        run: operation::RunFunction,
        operation_hash: OperationHash,
    ) -> Result<RunFunctionReceipt> {
        let operation::RunFunction {
            uri,
            method,
            headers,
            body,
            gas_limit,
        } = run;
        // 1. Extract address from request
        let address = Address::from_base58(uri.host().ok_or(Error::InvalidAddress)?)?;

        // 2. Initialize runtime with http api
        let rt = &mut Runtime::new(gas_limit)?;
        register_apis(rt, &address);

        // 3. Deserialize request
        let http_request = create_http_request(uri, method, headers, body)?;
        let request = JsNativeObject::new::<RequestClass>(
            Request::from_http_request(http_request, rt)?,
            rt,
        )?;

        // 4. Handle transfer if the header is present (without committing)
        {
            let mut guard = tx.lock();
            guard.begin();
        }
        let transfer_result = runtime::enter_js_host_context(hrt, tx.clone(), || {
            HostScript::handle_transfer(
                &mut request.deref().headers().deref_mut(),
                source,
                &address,
            )
        });
        if let Err(err) = transfer_result {
            tx.lock().rollback()?;
            return Err(err.into());
        }
        let is_noop = request.deref().url().path() == NOOP_PATH;
        // 5. For smart functions, execute the smart function, otherwise commit the transaction
        match address {
            Address::SmartFunction(sf_address) if !is_noop => {
                // Execute smart function
                let result = execute_smart_function(
                    hrt,
                    tx.clone(),
                    rt,
                    source,
                    // TODO: avoid cloning
                    // https://linear.app/tezos/issue/JSTZ-331/avoid-cloning-for-address-in-proto
                    sf_address.clone(),
                    request,
                    operation_hash,
                );
                debug_msg!(hrt, "🚀 Smart function executed successfully with value: {:?} (in {:?} instructions)\n", result, gas_limit - rt.instructions_remaining());
                // Commit or rollback based on the result
                let result = result.and_then(|response| {
                    runtime::enter_js_host_context(hrt, tx.clone(), || {
                        handle_refund(&response, &sf_address, source)
                    })
                });
                match &result {
                    Ok(receipt) if receipt.status_code.is_success() => {
                        tx.lock().commit(hrt)?
                    }
                    _ => tx.lock().rollback()?,
                }
                result
            }
            _ => {
                tx.lock().commit(hrt)?;
                Ok(RunFunctionReceipt::default())
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use http::{HeaderMap, Method};
        use jstz_core::kv::Transaction;
        use jstz_crypto::hash::Blake2b;
        use jstz_mock::host::JstzMockHost;
        use parking_lot::FairMutex as Mutex;
        use std::sync::Arc;

        use crate::{
            context::account::{Account, Address, ParsedCode},
            operation::RunFunction,
        };

        #[test]
        fn transfer_xtz_to_and_from_smart_function_succeeds() {
            let source = Address::User(jstz_mock::account1());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let transfer_amount = 3;
            let refund_amount = 2;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, transfer_amount)
                .expect("add balance");
            let source_balance = Account::balance(host, &mut tx.lock(), &source).unwrap();
            assert_eq!(source_balance, transfer_amount);
            tx.lock().commit(host).unwrap();

            // 1. Deploy the smart function that transfers the balance to the source
            let code = format!(
                r#"
            const handler = async (request) => {{
                const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
                if (transferred_amount !== "{transfer_amount}") {{
                    return Response.error("Invalid transferred amount");
                }}
                const headers = {{"X-JSTZ-TRANSFER": "{refund_amount}"}};
                return new Response(null, {{headers}});
            }};
            export default handler;
            "#
            );
            let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
            tx.lock().begin();
            let smart_function =
                Script::deploy(host, &mut tx.lock(), &source, parsed_code, 0).unwrap();

            let balance_before =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            assert_eq!(balance_before, 0);

            tx.lock().commit(host).unwrap();

            // 2. Call the smart function
            tx.lock().begin();
            let mut headers = HeaderMap::new();
            headers.insert(
                X_JSTZ_TRANSFER,
                transfer_amount.to_string().try_into().unwrap(),
            );
            let run_function = RunFunction {
                uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
                method: Method::GET,
                headers,
                body: None,
                gas_limit: 1000,
            };
            let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
            let response = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                fake_op_hash.clone(),
            )
            .expect("run function expected");

            assert!(response.headers.get(X_JSTZ_TRANSFER).is_none());
            assert!(response.headers.get(X_JSTZ_AMOUNT).is_some_and(|amt| amt
                .to_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                == refund_amount));
            tx.lock().commit(host).unwrap();

            // 3. assert the transfer to the sf and refund to the source
            tx.lock().begin();
            let balance_after =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            assert_eq!(
                balance_after - balance_before,
                transfer_amount - refund_amount
            );
            assert_eq!(
                Account::balance(host, &mut tx.lock(), &source).unwrap(),
                refund_amount
            );

            // 4. transferring to the smart function should fail (source has insufficient funds)
            let error = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                fake_op_hash.clone(),
            )
            .expect_err("Expected error");
            assert_eq!(
                error.to_string(),
                "EvalError: Transfer failed: InsufficientFunds"
            );

            // 5. transferring from the smart function should fail with insufficient funds and the balance is rolled back
            let balance_before = Account::balance(host, &mut tx.lock(), &source).unwrap();
            // drain the balance of the smart function
            Account::set_balance(host, &mut tx.lock(), &smart_function, 0).unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                X_JSTZ_TRANSFER,
                transfer_amount.to_string().try_into().unwrap(),
            );
            let error = execute(
                host,
                tx.clone(),
                &source,
                RunFunction {
                    headers,
                    ..run_function
                },
                fake_op_hash.clone(),
            )
            .expect_err("Expected error");
            let balance_after = Account::balance(host, &mut tx.lock(), &source).unwrap();
            assert_eq!(
                error.to_string(),
                "EvalError: Transfer failed: InsufficientFunds"
            );
            // tx rolled back as smart function has insufficient funds
            assert_eq!(balance_after, balance_before);
        }

        #[test]
        fn transfer_xtz_to_smart_function_succeeds_with_noop_path() {
            let source = Address::User(jstz_mock::account1());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let initial_balance = 1;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, initial_balance)
                .expect("add balance");
            let source_balance = Account::balance(host, &mut tx.lock(), &source).unwrap();
            assert_eq!(source_balance, initial_balance);
            tx.lock().commit(host).unwrap();

            // 1. Deploy the smart function that refunds the balance to the source
            let code = format!(
                r#"
                const handler = async () => {{
                    await fetch(new Request("jstz://{source}", {{
                        headers: {{"X-JSTZ-TRANSFER": "{initial_balance}"}}
                    }}));
                    return new Response();
                }};
                export default handler;
                "#
            );
            let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
            tx.lock().begin();
            let smart_function =
                Script::deploy(host, &mut tx.lock(), &source, parsed_code, 0).unwrap();

            let balance_before =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            assert_eq!(balance_before, 0);

            tx.lock().commit(host).unwrap();

            // transfer should happen with `/-/noop` path
            tx.lock().begin();
            let mut headers = HeaderMap::new();
            headers.insert(
                X_JSTZ_TRANSFER,
                initial_balance.to_string().try_into().unwrap(),
            );
            let run_function = RunFunction {
                uri: format!("jstz://{}/-/noop", &smart_function)
                    .try_into()
                    .unwrap(),
                method: Method::GET,
                headers,
                body: None,
                gas_limit: 1000,
            };
            let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
            execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                fake_op_hash,
            )
            .expect("run function expected");
            tx.lock().commit(host).unwrap();

            tx.lock().begin();
            let balance_after =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            assert_eq!(balance_after - balance_before, initial_balance);
            assert_eq!(Account::balance(host, &mut tx.lock(), &source).unwrap(), 0);
        }

        #[test]
        fn transfer_xtz_to_user_succeeds() {
            let source = Address::User(jstz_mock::account1());
            let destination = Address::User(jstz_mock::account2());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let initial_balance = 1;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, initial_balance)
                .expect("add balance");
            let source_balance = Account::balance(host, &mut tx.lock(), &source).unwrap();
            assert_eq!(source_balance, initial_balance);
            tx.lock().commit(host).unwrap();

            // 2. sending request to transfer from source to the destination
            tx.lock().begin();
            let mut headers = HeaderMap::new();
            headers.insert(
                X_JSTZ_TRANSFER,
                initial_balance.to_string().try_into().unwrap(),
            );
            let run_function = RunFunction {
                uri: format!("jstz://{}/", &destination).try_into().unwrap(),
                method: Method::GET,
                headers,
                body: None,
                gas_limit: 1000,
            };
            let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
            let result = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                fake_op_hash,
            );
            assert!(result.is_ok());

            tx.lock().commit(host).unwrap();

            tx.lock().begin();
            let balance_after = Account::balance(host, &mut tx.lock(), &source).unwrap();
            assert_eq!(balance_after, 0);
            assert_eq!(
                Account::balance(host, &mut tx.lock(), &destination).unwrap(),
                initial_balance
            );

            // 3. transferring again should fail
            let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
            let error = execute(host, tx.clone(), &source, run_function, fake_op_hash2)
                .expect_err("Expected error");
            assert_eq!(
                error.to_string(),
                "EvalError: Transfer failed: InsufficientFunds"
            );
        }

        #[test]
        fn invalid_request_should_fails() {
            let source = Address::User(jstz_mock::account1());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let initial_balance = 1;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, initial_balance)
                .expect("add balance");
            tx.lock().commit(host).unwrap();

            let code = r#"
                const handler = () => {{
                    return new Response();
                }};
                export default handler;
                "#;

            // 1. Deploy smart function
            let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
            tx.lock().begin();
            let smart_function =
                Script::deploy(host, &mut tx.lock(), &source, parsed_code, 0).unwrap();

            tx.lock().commit(host).unwrap();

            // Calling the smart function should error or return an error response
            tx.lock().begin();

            let sf_balance_before =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            let source_balance_before =
                Account::balance(host, &mut tx.lock(), &source).unwrap();
            let mut invalid_headers = HeaderMap::new();
            invalid_headers.insert(
                X_JSTZ_AMOUNT,
                initial_balance.to_string().try_into().unwrap(),
            );
            let run_function = RunFunction {
                uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
                method: Method::GET,
                headers: invalid_headers,
                body: None,
                gas_limit: 1000,
            };
            let result = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                Blake2b::from(b"fake_op_hash".as_ref()),
            );
            let sf_balance_after =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            let source_balance_after =
                Account::balance(host, &mut tx.lock(), &source).unwrap();

            assert_eq!(sf_balance_before, sf_balance_after);
            assert_eq!(source_balance_before, source_balance_after);
            let call_failed = match result {
                Ok(receipt) => receipt.status_code.is_server_error(),
                _ => true,
            };
            assert!(call_failed);
        }

        #[test]
        fn invalid_response_should_fails() {
            let source = Address::User(jstz_mock::account1());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let initial_balance = 1;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, initial_balance)
                .expect("add balance");
            tx.lock().commit(host).unwrap();

            let code = format!(
                r#"
                const handler = () => {{
                    const headers = new Headers();
                    return new Response(null, {{
                        headers: {{ "X-JSTZ-AMOUNT": "{initial_balance}" }},
                    }});
                }};
                export default handler;
                "#
            );

            // 1. Deploy smart function
            let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
            tx.lock().begin();
            let smart_function = Script::deploy(
                host,
                &mut tx.lock(),
                &source,
                parsed_code,
                initial_balance,
            )
            .unwrap();

            let sf_balance_before =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            let source_balance_before =
                Account::balance(host, &mut tx.lock(), &source).unwrap();

            tx.lock().commit(host).unwrap();

            // Calling the smart function should error or return an error response
            tx.lock().begin();
            let run_function = RunFunction {
                uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
                method: Method::GET,
                headers: Default::default(),
                body: None,
                gas_limit: 1000,
            };
            let result = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                Blake2b::from(b"fake_op_hash".as_ref()),
            );
            let sf_balance_after =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            let source_balance_after =
                Account::balance(host, &mut tx.lock(), &source).unwrap();

            assert_eq!(sf_balance_before, sf_balance_after);
            assert_eq!(source_balance_before, source_balance_after);
            let call_failed = match result {
                Ok(receipt) => receipt.status_code.is_server_error(),
                _ => true,
            };
            assert!(call_failed);
        }

        #[test]
        fn transfer_xtz_and_smart_function_call_is_atomic1() {
            let invalid_code = r#"
           const handler = () => {{
               invalid();
           }};
           export default handler;
           "#;
            transfer_xtz_and_run_erroneous_sf(invalid_code);
        }

        #[test]
        fn transfer_xtz_and_smart_function_call_is_atomic2() {
            let invalid_code = r#"
           const handler = () => {{
                return Response.error("error!");
           }};
           export default handler;
           "#;
            transfer_xtz_and_run_erroneous_sf(invalid_code);
        }

        #[test]
        fn transfer_xtz_and_smart_function_call_is_atomic3() {
            let invalid_code = r#"
           const handler = () => {{
            return 3;
           }};
           export default handler;
           "#;
            transfer_xtz_and_run_erroneous_sf(invalid_code);
        }

        fn transfer_xtz_and_run_erroneous_sf(code: &str) {
            let source = Address::User(jstz_mock::account1());
            // 1. Deploy the smart function
            let mut jstz_mock_host = JstzMockHost::default();
            let host = jstz_mock_host.rt();
            let tx = Arc::new(Mutex::new(Transaction::default()));
            let initial_balance = 1;
            tx.lock().begin();
            Account::add_balance(host, &mut tx.lock(), &source, initial_balance)
                .expect("add balance");
            tx.lock().commit(host).unwrap();

            // 1. Deploy smart function
            let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
            tx.lock().begin();
            let smart_function =
                Script::deploy(host, &mut tx.lock(), &source, parsed_code, 0).unwrap();

            tx.lock().commit(host).unwrap();

            // Calling the smart function should error or return an error response
            tx.lock().begin();
            let mut headers = HeaderMap::new();
            headers.insert(
                X_JSTZ_TRANSFER,
                initial_balance.to_string().try_into().unwrap(),
            );
            let run_function = RunFunction {
                uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
                method: Method::GET,
                headers,
                body: None,
                gas_limit: 1000,
            };
            let result = execute(
                host,
                tx.clone(),
                &source,
                run_function.clone(),
                Blake2b::from(b"fake_op_hash".as_ref()),
            );
            let call_failed = match result {
                Ok(receipt) => receipt.status_code.is_server_error(),
                _ => true,
            };
            assert!(call_failed);

            // The balance should not be affected
            assert_eq!(
                Account::balance(host, &mut tx.lock(), &source).unwrap(),
                initial_balance
            );
            let balance_after =
                Account::balance(host, &mut tx.lock(), &smart_function).unwrap();
            assert_eq!(balance_after, 0);
        }
    }
}

pub mod jstz_run {
    use jstz_core::kv::Storage;
    use serde::Deserialize;
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup::storage::path::{OwnedPath, RefPath};

    use super::*;
    use crate::{
        executor::{fa_withdraw::FaWithdraw, withdraw::Withdrawal, JSTZ_HOST},
        operation::RunFunction,
        receipt,
    };

    const WITHDRAW_PATH: &str = "/withdraw";
    const FA_WITHDRAW_PATH: &str = "/fa-withdraw";

    fn validate_withdraw_request<'de, T>(run: &'de RunFunction) -> Result<T>
    where
        T: Deserialize<'de>,
    {
        let method = run
            .method
            .as_str()
            .parse::<http::Method>()
            .map_err(|_| Error::InvalidHttpRequestMethod)?;

        if method != http::Method::POST {
            return Err(Error::InvalidHttpRequestMethod);
        }

        if run.body.is_none() {
            return Err(Error::InvalidHttpRequestBody);
        }
        let withdrawal = serde_json::from_slice(run.body.as_ref().unwrap())
            .map_err(|_| Error::InvalidHttpRequestBody)?;
        Ok(withdrawal)
    }

    pub fn execute(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        ticketer: &ContractKt1Hash,
        source: &impl Addressable,
        run: RunFunction,
    ) -> Result<receipt::RunFunctionReceipt> {
        let uri = run.uri.clone();
        if uri.host() != Some(JSTZ_HOST) {
            return Err(Error::InvalidHost);
        }
        match uri.path() {
            WITHDRAW_PATH => {
                // TODO: https://linear.app/tezos/issue/JSTZ-77/check-gas-limit-when-performing-native-withdraws
                // Check gas limit

                let withdrawal = validate_withdraw_request::<Withdrawal>(&run)?;
                crate::executor::withdraw::execute_withdraw(
                    hrt, tx, source, withdrawal, ticketer,
                )?;
                let receipt = receipt::RunFunctionReceipt {
                    body: None,
                    status_code: http::StatusCode::OK,
                    headers: http::HeaderMap::new(),
                };
                Ok(receipt)
            }
            FA_WITHDRAW_PATH => {
                let fa_withdraw = validate_withdraw_request::<FaWithdraw>(&run)?;
                let fa_withdraw_receipt_content = fa_withdraw.execute(
                    hrt, tx, source, 1000, // fake gas limit
                )?;
                let receipt = receipt::RunFunctionReceipt {
                    body: fa_withdraw_receipt_content.to_http_body(),
                    status_code: http::StatusCode::OK,
                    headers: http::HeaderMap::new(),
                };
                Ok(receipt)
            }
            _ => Err(Error::UnsupportedPath),
        }
    }

    pub fn execute_without_ticketer(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &impl Addressable,
        run: RunFunction,
    ) -> Result<receipt::RunFunctionReceipt> {
        let ticketer_path = OwnedPath::from(&RefPath::assert_from(b"/ticketer"));
        let ticketer: SmartFunctionHash =
            Storage::get(hrt, &ticketer_path)?.expect("ticketer should be set");
        execute(hrt, tx, &ticketer, source, run)
    }

    #[cfg(test)]
    mod test {
        use http::{header, HeaderMap, Method, Uri};
        use jstz_core::kv::Transaction;
        use jstz_crypto::hash::Hash;
        use jstz_mock::host::JstzMockHost;
        use serde_json::json;
        use tezos_crypto_rs::hash::ContractKt1Hash;
        use tezos_smart_rollup_mock::MockHost;

        use crate::{
            context::{account::Address, ticket_table::TicketTable},
            executor::{
                fa_withdraw::{FaWithdraw, RoutingInfo, TicketInfo},
                smart_function::jstz_run::{execute_without_ticketer, Account},
            },
            operation::RunFunction,
            Error,
        };

        use super::execute;

        fn withdraw_request() -> RunFunction {
            RunFunction {
                uri: Uri::try_from("jstz://jstz/withdraw").unwrap(),
                method: Method::POST,
                headers: HeaderMap::from_iter([(
                    header::CONTENT_TYPE,
                    "application/json".try_into().unwrap(),
                )]),
                body: Some(
                    json!({
                        "amount": 10,
                        "receiver": jstz_mock::account2().to_base58().to_string(),
                    })
                    .to_string()
                    .as_bytes()
                    .to_vec(),
                ),
                gas_limit: 10,
            }
        }

        fn fa_withdraw_request() -> RunFunction {
            let ticket_info = TicketInfo {
                id: 1234,
                content: Some(b"random ticket content".to_vec()),
                ticketer: jstz_mock::kt1_account1().into(),
            };
            let routing_info = RoutingInfo {
                receiver: Address::User(jstz_mock::account2()),
                proxy_l1_contract: jstz_mock::kt1_account1().into(),
            };
            let fa_withdrawal = FaWithdraw {
                amount: 10,
                routing_info,
                ticket_info,
            };

            RunFunction {
                uri: Uri::try_from("jstz://jstz/fa-withdraw").unwrap(),
                method: Method::POST,
                headers: HeaderMap::from_iter([(
                    header::CONTENT_TYPE,
                    "application/json".try_into().unwrap(),
                )]),
                body: Some(json!(fa_withdrawal).to_string().as_bytes().to_vec()),
                gas_limit: 10,
            }
        }

        #[test]
        fn execute_fails_on_invalid_host() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                uri: Uri::try_from("jstz://example.com/withdraw").unwrap(),
                ..withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(super::Error::InvalidHost)));
        }

        #[test]
        fn execute_fails_on_unsupported_path() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                uri: Uri::try_from("jstz://jstz/blahblah").unwrap(),
                ..withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(super::Error::UnsupportedPath)));
        }

        #[test]
        fn execute_wthdraw_fails_on_invalid_request_method() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                method: Method::GET,
                ..withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(
                result,
                Err(super::Error::InvalidHttpRequestMethod)
            ));
        }

        #[test]
        fn execute_wthdraw_fails_on_invalid_request_body() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                body: Some(
                    json!({
                        "amount": 10,
                        "not_receiver": jstz_mock::account2().to_base58()
                    })
                    .to_string()
                    .as_bytes()
                    .to_vec(),
                ),
                ..withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));

            let req = RunFunction {
                body: None,
                ..withdraw_request()
            };
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));
        }

        #[test]
        fn execute_withdraw_succeeds() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());

            tx.begin();
            Account::add_balance(&host, &mut tx, &source, 10).unwrap();
            tx.commit(&mut host).unwrap();

            let req = withdraw_request();
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();

            execute(&mut host, &mut tx, &ticketer, &source, req)
                .expect("Withdraw should not fail");

            tx.begin();
            assert_eq!(0, Account::balance(&host, &mut tx, &source).unwrap());

            let level = host.run_level(|_| {});
            assert_eq!(1, host.outbox_at(level).len());
        }

        #[test]
        fn execute_without_ticketer_succeeds() {
            let mut host = JstzMockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let rt = host.rt();

            tx.begin();
            Account::add_balance(rt, &mut tx, &source, 10).unwrap();
            tx.commit(rt).unwrap();

            let req = withdraw_request();

            execute_without_ticketer(rt, &mut tx, &source, req)
                .expect("Withdraw should not fail");

            tx.begin();
            assert_eq!(0, Account::balance(rt, &mut tx, &source).unwrap());

            let level = rt.run_level(|_| {});
            assert_eq!(1, rt.outbox_at(level).len());
        }

        #[test]
        fn execute_fa_withdraw_fails_on_invalid_request_method() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                method: Method::GET,
                ..fa_withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(
                result,
                Err(super::Error::InvalidHttpRequestMethod)
            ));
        }

        #[test]
        fn execute_fa_withdraw_fails_on_invalid_request_body() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let req = RunFunction {
                body: Some(
                    json!({
                        "amount": 10,
                        "not_receiver": jstz_mock::account2().to_base58()
                    })
                    .to_string()
                    .as_bytes()
                    .to_vec(),
                ),
                ..fa_withdraw_request()
            };
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));

            let req = RunFunction {
                body: None,
                ..withdraw_request()
            };
            let result = execute(&mut host, &mut tx, &ticketer, &source, req);
            assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));
        }

        #[test]
        fn execute_fa_withdraw_succeeds() {
            let mut host = MockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());

            let ticket = TicketInfo {
                id: 1234,
                content: Some(b"random ticket content".to_vec()),
                ticketer: jstz_mock::kt1_account1().into(),
            }
            .to_ticket(1)
            .unwrap();

            tx.begin();
            TicketTable::add(&mut host, &mut tx, &source, &ticket.hash, 10).unwrap();
            tx.commit(&mut host).unwrap();

            let req = fa_withdraw_request();
            let ticketer =
                ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER)
                    .unwrap();

            execute(&mut host, &mut tx, &ticketer, &source, req)
                .expect("Withdraw should not fail");

            tx.begin();
            assert_eq!(0, Account::balance(&host, &mut tx, &source).unwrap());

            let level = host.run_level(|_| {});
            assert_eq!(1, host.outbox_at(level).len());
        }
    }
}

pub mod deploy {
    use super::*;
    use crate::{operation, receipt};

    pub fn execute(
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &impl Addressable,
        deployment: operation::DeployFunction,
    ) -> Result<receipt::DeployFunctionReceipt> {
        let operation::DeployFunction {
            function_code,
            account_credit,
        } = deployment;

        let address = Script::deploy(hrt, tx, source, function_code, account_credit)?;

        Ok(receipt::DeployFunctionReceipt { address })
    }

    #[cfg(test)]
    mod test {
        use crate::context::account::Address;

        use super::*;
        use jstz_core::kv::Transaction;
        use jstz_mock::host::JstzMockHost;
        use operation::DeployFunction;

        #[test]
        fn execute_deploy_deploys_smart_function_with_kt1_account1() {
            let mut host = JstzMockHost::default();
            let mut tx = Transaction::default();
            let source = Address::User(jstz_mock::account1());
            let hrt = host.rt();
            tx.begin();

            let deployment = DeployFunction {
                function_code: "".to_string().try_into().unwrap(),
                account_credit: 0,
            };
            let result = deploy::execute(hrt, &mut tx, &source, deployment);
            assert!(result.is_ok());
            let receipt = result;
            assert!(receipt.is_ok());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use boa_engine::Context;
    use http::header::{HeaderName, HeaderValue};
    use jstz_api::http::header::HeadersClass;
    use jstz_core::native::register_global_class;

    fn create_test_request(headers: Vec<(String, String)>) -> JsResult<Request> {
        let mut context = Context::default();
        register_global_class::<RequestClass>(&mut context)?;
        register_global_class::<HeadersClass>(&mut context)?;

        let mut builder = http::Request::builder()
            .method("POST")
            .uri("jstz://test")
            .body(Some(Vec::new()))
            .map_err(|e| {
                JsError::from_native(
                    JsNativeError::error()
                        .with_message(format!("Failed to create request: {}", e)),
                )
            })?;

        // Set headers after building
        let headers_map = builder.headers_mut();
        for (key, value) in headers {
            headers_map.insert(
                HeaderName::from_str(&key).map_err(|e| {
                    JsError::from_native(
                        JsNativeError::error()
                            .with_message(format!("Invalid header name: {}", e)),
                    )
                })?,
                HeaderValue::from_str(&value).map_err(|e| {
                    JsError::from_native(
                        JsNativeError::error()
                            .with_message(format!("Invalid header value: {}", e)),
                    )
                })?,
            );
        }

        Request::from_http_request(builder, &mut context)
    }

    mod transfer_amount {
        use super::*;
        use std::ops::Deref;

        struct TestRequest(Request);

        impl Deref for TestRequest {
            type Target = Request;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        fn wrap_request(request: Request) -> TestRequest {
            TestRequest(request)
        }

        #[test]
        fn test_valid_amount() -> JsResult<()> {
            let request = wrap_request(create_test_request(vec![(
                X_JSTZ_TRANSFER.to_string(),
                "1000".to_string(),
            )])?);
            assert_eq!(
                HostScript::extract_transfer_amount(&request.headers().deref())?,
                Some(NonZeroU64::new(1000).unwrap())
            );
            Ok(())
        }

        #[test]
        fn test_missing_header() -> JsResult<()> {
            let request = wrap_request(create_test_request(vec![])?);
            assert_eq!(
                HostScript::extract_transfer_amount(&request.headers().deref())?,
                None
            );
            Ok(())
        }

        #[test]
        fn test_invalid_amount() -> JsResult<()> {
            let request = wrap_request(create_test_request(vec![(
                X_JSTZ_TRANSFER.to_string(),
                "invalid".to_string(),
            )])?);
            assert!(
                HostScript::extract_transfer_amount(&request.headers().deref()).is_err()
            );
            Ok(())
        }
    }
}
