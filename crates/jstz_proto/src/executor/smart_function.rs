use std::ops::BitXor;

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
        request::{Request, RequestClass},
        response::{Response, ResponseClass, ResponseOptions},
    },
    js_log::set_js_logger,
};
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, native::JsNativeObject, runtime,
    Module, Realm,
};
use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    api::{self, TraceData},
    context::{
        account::{Account, Amount, ParsedCode},
        new_account::NewAddress,
    },
    js_logger::JsonLogger,
    operation::{OperationHash, RunFunction},
    receipt,
    request_logger::{log_request_end, log_request_start},
    Error, Result,
};

pub mod headers {

    use super::*;
    pub const REFERRER: &str = "Referer";

    pub fn test_and_set_referrer(
        request: &Request,
        referer: &NewAddress,
    ) -> JsResult<()> {
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

// Applies on_fullfilled or on_rejected based on either an error was raised or not.
// If the value is a promise, then we apply the on_fulfilled and on_rejected to the promise.
fn try_apply_to_value_or_promise(
    value_or_promise: JsResult<JsValue>,
    on_fulfilled: fn(&JsValue, &mut Context) -> JsResult<()>,
    on_rejected: fn(&mut Context) -> JsResult<()>,
    context: &mut Context,
) -> JsResult<JsValue> {
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

fn compute_seed(address: &NewAddress, operation_hash: &OperationHash) -> u64 {
    let mut seed: u64 = 0;
    for byte in operation_hash.as_array().iter().chain(address.as_bytes()) {
        seed = seed.rotate_left(8).bitxor(*byte as u64)
    }

    seed
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
    address: &NewAddress,
    seed: u64,
    context: &mut Context,
) {
    // TODO: remove once smart function address is supported
    // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
    let pkh = match address {
        NewAddress::User(pkh) => pkh,
        _ => panic!("Smart function address is not supported yet"),
    };
    realm.register_api(
        jstz_api::KvApi {
            address: pkh.clone(),
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
        address: &NewAddress,
        context: &mut Context,
    ) -> Result<Self> {
        let src =
            Account::function_code(hrt, tx, address)?.ok_or(Error::InvalidAddress)?;

        Ok(Self::parse(Source::from_bytes(&src), context)?)
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
        address: &NewAddress,
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
        address: &NewAddress,
        operation_hash: &OperationHash,
        context: &mut Context,
    ) -> JsPromise {
        self.register_apis(address, operation_hash, context);

        self.realm().eval_module(self, context)
    }

    /// Deploys a script
    pub fn deploy(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        source: &NewAddress,
        code: ParsedCode,
        balance: Amount,
    ) -> Result<NewAddress> {
        let nonce = Account::nonce(hrt, tx, source)?;

        // TODO: use sf address
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        let address = NewAddress::User(PublicKeyHash::digest(
            format!("{}{}{}", source, code, nonce).as_bytes(),
        )?);

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
        address: &NewAddress,
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
            |_context| Ok(runtime::with_js_tx(|tx| tx.rollback())?),
            context,
        )
    }

    /// Loads, initializes and runs the script
    pub fn load_init_run(
        address: NewAddress,
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
        run_receipt: receipt::RunFunctionReceipt,
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
        self_address: &NewAddress,
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
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        source: &NewAddress,
        run: operation::RunFunction,
        operation_hash: OperationHash,
    ) -> Result<receipt::RunFunctionReceipt> {
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
        //TODO: check if this is sf address
        let address = NewAddress::from_base58(uri.host().ok_or(Error::InvalidAddress)?)?;

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

        debug_msg!(
            hrt,
            "🚀 Smart function executed successfully with value: {:?} (in {:?} instructions)\n", result, gas_limit - rt.instructions_remaining()
        );

        // 6. Serialize response
        let response = Response::try_from_js(&result)?;

        let (http_parts, body) = Response::to_http_response(&response).into_parts();

        Ok(receipt::RunFunctionReceipt {
            body,
            status_code: http_parts.status,
            headers: http_parts.headers,
        })
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
        source: &NewAddress,
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
        source: &NewAddress,
        run: RunFunction,
    ) -> Result<receipt::RunFunctionReceipt> {
        let ticketer_path = OwnedPath::from(&RefPath::assert_from(b"/ticketer"));
        let ticketer: ContractKt1Hash =
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
            context::ticket_table::TicketTable,
            executor::{
                fa_withdraw::{FaWithdraw, RoutingInfo, TicketInfo},
                smart_function::jstz_run::{execute_without_ticketer, Account},
            },
            operation::RunFunction,
            Error,
        };

        use super::{execute, NewAddress};

        fn withdraw_request() -> RunFunction {
            RunFunction {
                uri: Uri::try_from("tezos://jstz/withdraw").unwrap(),
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
                ticketer: jstz_mock::kt1_account1(),
            };
            let routing_info = RoutingInfo {
                receiver: NewAddress::User(jstz_mock::account2()),
                proxy_l1_contract: jstz_mock::kt1_account1(),
            };
            let fa_withdrawal = FaWithdraw {
                amount: 10,
                routing_info,
                ticket_info,
            };

            RunFunction {
                uri: Uri::try_from("tezos://jstz/fa-withdraw").unwrap(),
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
            let source = NewAddress::User(jstz_mock::account1());
            let req = RunFunction {
                uri: Uri::try_from("tezos://example.com/withdraw").unwrap(),
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
            let source = NewAddress::User(jstz_mock::account1());
            let req = RunFunction {
                uri: Uri::try_from("tezos://jstz/blahblah").unwrap(),
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
            let source = NewAddress::User(jstz_mock::account1());
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
            let source = NewAddress::User(jstz_mock::account1());
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
            let source = NewAddress::User(jstz_mock::account1());

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
            let source = NewAddress::User(jstz_mock::account1());
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
            let source = NewAddress::User(jstz_mock::account1());
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
            let source = NewAddress::User(jstz_mock::account1());
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
            let source = NewAddress::User(jstz_mock::account1());

            let ticket = TicketInfo {
                id: 1234,
                content: Some(b"random ticket content".to_vec()),
                ticketer: jstz_mock::kt1_account1(),
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
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        source: &NewAddress,
        deployment: operation::DeployFunction,
    ) -> Result<receipt::DeployFunctionReceipt> {
        let operation::DeployFunction {
            function_code,
            account_credit,
        } = deployment;

        let address = Script::deploy(hrt, tx, source, function_code, account_credit)?;

        Ok(receipt::DeployFunctionReceipt { address })
    }
}
