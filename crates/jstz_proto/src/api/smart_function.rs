use boa_engine::{
    js_string,
    object::{builtins::JsPromise, ErasedObject, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsData, JsNativeError, JsResult, JsValue, NativeFunction,
};

use jstz_api::http::request::Request;
use jstz_core::{
    host::HostRuntime,
    host_defined,
    kv::Transaction,
    native::JsNativeObject,
    runtime::{self},
    value::IntoJs,
};
use jstz_crypto::smart_function_hash::SmartFunctionHash;

use crate::{
    context::account::{Account, Amount, ParsedCode},
    executor::smart_function,
    operation::{DeployFunction, OperationHash},
    Result,
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

#[derive(JsData)]
pub struct TraceData {
    pub address: SmartFunctionHash,
    pub operation_hash: OperationHash,
}

impl Finalize for TraceData {}

unsafe impl Trace for TraceData {
    empty_trace!();
}

#[derive(JsData)]
struct SmartFunction {
    address: SmartFunctionHash,
}
impl Finalize for SmartFunction {}

unsafe impl Trace for SmartFunction {
    empty_trace!();
}

impl SmartFunction {
    fn from_js_value(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `SmartFunction`",
                    )
                    .into()
            })
    }

    fn create(
        &self,
        hrt: &mut impl HostRuntime,
        tx: &mut Transaction,
        function_code: ParsedCode,
        initial_balance: Amount,
    ) -> Result<String> {
        // 1. Deploy the smart function
        let deploy_receipt = smart_function::deploy::execute(
            hrt,
            tx,
            &self.address,
            DeployFunction {
                function_code,
                account_credit: initial_balance,
            },
        )?;

        // 2. Increment nonce of current account
        Account::nonce(hrt, tx, &self.address)?.increment();

        Ok(deploy_receipt.address.to_string())
    }

    // Invariant: The function should always be called within a js_host_context
    fn call(
        self_address: &SmartFunctionHash,
        request: &JsNativeObject<Request>,
        operation_hash: OperationHash,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        smart_function::run::fetch(self_address, operation_hash, request, context)
    }
}

pub struct SmartFunctionApi {
    pub address: SmartFunctionHash,
}

impl SmartFunctionApi {
    const NAME: &'static str = "SmartFunction";

    fn fetch(
        address: &SmartFunctionHash,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        host_defined!(context, host_defined);
        let trace_data = host_defined
            .get::<TraceData>()
            .expect("trace data undefined");

        let request: JsNativeObject<Request> =
            args.get_or_undefined(0).clone().try_into()?;

        SmartFunction::call(
            address,
            &request,
            trace_data.operation_hash.clone(),
            context,
        )
    }

    fn call(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let smart_function = SmartFunction::from_js_value(this)?;
        Self::fetch(&smart_function.address, args, context)
    }

    fn create(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let smart_function = SmartFunction::from_js_value(this)?;

        let function_code: String = args
            .first()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Expected at least 1 argument but 0 provided")
            })?
            .try_js_into(context)?;
        let parsed_code: ParsedCode = function_code.try_into()?;

        let initial_balance = match args.get(1) {
            None => 0,
            Some(balance) => balance.to_big_uint64(context)?,
        };

        let promise = JsPromise::new(
            move |resolvers, context| {
                let address = runtime::with_js_hrt_and_tx(|hrt, tx| {
                    smart_function.create(hrt, tx, parsed_code, initial_balance as Amount)
                })?;

                resolvers.resolve.call(
                    &JsValue::undefined(),
                    &[address.into_js(context)],
                    context,
                )?;
                Ok(JsValue::undefined())
            },
            context,
        );

        Ok(promise.into())
    }
}

impl jstz_core::Api for SmartFunctionApi {
    fn init(self, context: &mut Context) {
        let smart_function = ObjectInitializer::with_native_data(
            SmartFunction {
                address: self.address.clone(),
            },
            context,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::call),
            js_string!("call"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(Self::create),
            js_string!("create"),
            2,
        )
        .build();

        context
            .register_global_property(
                js_string!(Self::NAME),
                smart_function,
                Attribute::all(),
            )
            .expect("The smart function object shouldn't exist yet");

        context
            .register_global_builtin_callable(
                js_string!("fetch"),
                1,
                NativeFunction::from_copy_closure_with_captures(
                    |_, args, this, ctx| Self::fetch(&this.address, args, ctx),
                    SmartFunction {
                        address: self.address,
                    },
                ),
            )
            .expect("The fetch function shouldn't exist yet");
    }
}

#[cfg(test)]
mod test {

    use http::{HeaderMap, Method};
    use jstz_api::http::request::{Request, RequestClass};
    use jstz_core::{
        kv::Transaction,
        native::JsNativeObject,
        runtime::{self, with_js_hrt_and_tx},
        Runtime,
    };
    use jstz_crypto::{
        hash::{Blake2b, Hash},
        public_key_hash::PublicKeyHash,
        smart_function_hash::SmartFunctionHash,
    };
    use jstz_mock::host::JstzMockHost;
    use serde_json::json;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{
            account::{Account, Address, ParsedCode},
            ticket_table::TicketTable,
        },
        executor::smart_function::{self, register_web_apis, X_JSTZ_TRANSFER},
        operation::{OperationHash, RunFunction},
    };

    use super::SmartFunction;

    #[test]
    fn call_system_script_succeeds() {
        let mut mock_host = JstzMockHost::default();
        let rt = mock_host.rt();

        let mut jstz_rt = Runtime::new(10000).unwrap();
        let realm = jstz_rt.realm().clone();
        let context = jstz_rt.context();

        register_web_apis(&realm, context);

        let self_address = SmartFunctionHash::digest(b"random bytes").unwrap();

        let amount = 100;

        let operation_hash = Blake2b::from(b"operation_hash".as_ref());
        let receiver = Address::User(PublicKeyHash::digest(b"receiver address").unwrap());
        let http_request = http::Request::builder()
            .method(Method::POST)
            .uri("jstz://jstz/withdraw")
            .header("Content-type", "application/json")
            .body(Some(
                json!({
                    "receiver": receiver,
                    "amount": 100
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            ))
            .unwrap();

        let request = Request::from_http_request(http_request, context).unwrap();

        let mut tx = Transaction::default();
        runtime::enter_js_host_context(rt, &mut tx, || {
            with_js_hrt_and_tx(|hrt, tx| {
                tx.begin();
                Account::add_balance(hrt, tx, &self_address, amount).unwrap();
                tx.commit(hrt).unwrap();
            });

            SmartFunction::call(
                &self_address,
                &JsNativeObject::new::<RequestClass>(request, context).unwrap(),
                operation_hash,
                context,
            )
            .unwrap();
        });
    }

    #[test]
    fn host_script_withdraw_from_smart_function_succeeds() {
        let mut mock_host = JstzMockHost::default();
        let host = mock_host.rt();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let code = r#"
        export default (request) => {
            const withdrawRequest = new Request("jstz://jstz/withdraw", {
                method: "POST",
                headers: {
                    "Content-type": "application/json",
                },
                body: JSON.stringify({
                    receiver: "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
                    amount: 5,
                }),
            });
            return SmartFunction.call(withdrawRequest);
        }
        "#;
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        Account::add_balance(host, &mut tx, &source, 1000).unwrap();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 5).unwrap();
        tx.commit(host).unwrap();

        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .expect("Withdrawal expected to succeed");
        tx.commit(host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(1, outbox.len());

        // Trying to withdraw again should fail with insufficient funds
        tx.begin();
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let error = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function,
            fake_op_hash2,
        )
        .expect_err("Expected error");
        assert_eq!("EvalError: InsufficientFunds", error.to_string());
    }

    #[test]
    fn transfer_xtz_from_smart_function_succeeds() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let transfer_amount: u64 = 1;

        let smart_function1 = deploy_transfer_sf_and_execute(
            source.clone(),
            host,
            &mut tx,
            transfer_amount,
        );
        // deploy a new smart function that transfers balance to a smart function address
        let code2 = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{smart_function1}/", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code2 = ParsedCode::try_from(code2.to_string()).unwrap();
        tx.begin();
        let smart_function2 =
            smart_function::deploy(host, &mut tx, &source, parsed_code2, transfer_amount)
                .unwrap();

        // 6. Call the new smart function
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function2).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let source_before = Account::balance(host, &mut tx, &source).unwrap();
        smart_function::run::execute(host, &mut tx, &source, run_function, fake_op_hash2)
            .unwrap();
        tx.commit(host).unwrap();
        tx.begin();
        let source_after = Account::balance(host, &mut tx, &source).unwrap();
        // 7. Assert sf2 transferred to sf1
        assert_eq!(
            Account::balance(host, &mut tx, &smart_function2).unwrap(),
            0
        );
        assert_eq!(source_after - source_before, transfer_amount);
    }

    #[test]
    fn transfer_xtz_from_smart_function_succeeds_with_noop() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx: Transaction = Transaction::default();
        let transfer_amount: u64 = 1;
        tx.begin();
        tx.commit(host).unwrap();
        // deploy and execute smart function that transfers `transfer_amount` to the `source`
        let smart_function = deploy_transfer_sf_and_execute(
            source.clone(),
            host,
            &mut tx,
            transfer_amount,
        );

        // deploy a new smart function that transfers balance to a smart function address
        // without executing the sf using /-/noop path
        let code2 = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{smart_function}/-/noop", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code2 = ParsedCode::try_from(code2.to_string()).unwrap();
        tx.begin();
        let smart_function2 =
            smart_function::deploy(host, &mut tx, &source, parsed_code2, transfer_amount)
                .unwrap();

        // calling the smart function2
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function2).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let source_before = Account::balance(host, &mut tx, &source).unwrap();
        let sf2_before = Account::balance(host, &mut tx, &smart_function2).unwrap();
        smart_function::run::execute(host, &mut tx, &source, run_function, fake_op_hash2)
            .unwrap();
        tx.commit(host).unwrap();
        // the source shouldn't received balance as sf1 isn't executed
        tx.begin();
        let source_after = Account::balance(host, &mut tx, &source).unwrap();
        let sf2_after = Account::balance(host, &mut tx, &smart_function2).unwrap();
        assert_eq!(source_after, source_before);
        assert_eq!(sf2_before - sf2_after, transfer_amount);
    }

    // deploy a smart function that transfers `transfer_amount` to the `source`
    // and executes it. returns the executed smart function address
    fn deploy_transfer_sf_and_execute(
        source: Address,
        host: &mut MockHost,
        tx: &mut Transaction,
        transfer_amount: u64,
    ) -> SmartFunctionHash {
        let initial_sf_balance: u64 = 1_028_230_587 * 1_000_000;
        tx.begin();
        Account::add_balance(host, tx, &source, initial_sf_balance).unwrap();
        tx.commit(host).unwrap();

        // 1. Deploy the smart function that transfers the balance to user address
        let code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "{transfer_amount}");
                await fetch(new Request("jstz://{source}", {{
                    headers: myHeaders
                }}));
                return new Response();
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let smart_function = smart_function::deploy(
            host,
            tx,
            &source,
            parsed_code.clone(),
            initial_sf_balance,
        )
        .unwrap();

        let balance_before = Account::balance(host, tx, &smart_function).unwrap();
        assert_eq!(balance_before, initial_sf_balance);

        tx.commit(host).unwrap();

        // 2. Call the smart function
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .expect("run function expected");
        tx.commit(host).unwrap();

        // 3. Assert the transfer from the smart function to the user address
        tx.begin();
        let balance_after = Account::balance(host, tx, &smart_function).unwrap();
        assert_eq!(balance_before - balance_after, transfer_amount);
        assert_eq!(
            Account::balance(host, tx, &source).unwrap(),
            transfer_amount
        );
        tx.commit(host).unwrap();
        smart_function
    }

    #[test]
    fn failure_on_transfer_xtz_from_smart_function_returns_error_response() {
        let source = Address::User(jstz_mock::account2());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // 2. Deploy the smart function that transfers the balance to the source
        let code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-TRANSFER", "1");
                return await fetch(new Request("jstz://{source}", {{
                    headers: myHeaders
                }}));
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // 3. Calling the smart function with insufficient funds should result in an error response
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let receipt = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .expect("run function expected receipt");

        assert!(receipt.status_code.is_server_error());
    }

    #[test]
    fn smart_function_refund_can_propagate() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(refund_code.to_string()).unwrap();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function and propagates the response
        let code = format!(
            r#"
            const handler = async() => {{
                const response = await fetch(new Request("jstz://{refund_sf}"));
                const refunded = response.headers.get("X-JSTZ-AMOUNT");
                // propagate the refunded amount to the caller
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": refunded }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .expect("run function expected");
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();
        tx.commit(host).unwrap();

        // 4. Assert the refund is propagated to the source instead of the caller_sf
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source + refund_amount, balance_after_source);
    }

    #[test]
    fn propagating_smart_function_refund_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(refund_code.to_string()).unwrap();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let code = format!(
            r#"
            const handler = () => {{
                return fetch(new Request("jstz://{refund_sf}"));
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .unwrap();
        assert!(result.status_code.is_server_error());

        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    #[test]
    fn returning_invalid_refund_amount_in_response_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_amount = 1;
        let invalid_refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-AMOUNT": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(invalid_refund_code.to_string()).unwrap();
        let fake_refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let code = format!(
            r#"
            const handler = async () => {{
                const response = await fetch(new Request("jstz://{fake_refund_sf}"));
                if (response.ok) {{
                    return new Response(); 
                }} else {{
                    return Response.error();
                }}
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .unwrap();
        assert!(result.status_code.is_server_error());

        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    #[test]
    fn returning_invalid_request_amount_fails() {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 1;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(
            host,
            &mut tx,
            &source,
            initial_refund_sf_balance + initial_caller_sf_balance,
        )
        .unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let refund_code = r#"
            const handler = () => {
                return new Response(null, {
                    headers: { "X-JSTZ-TRANSFER": "1" },
                });
            };
            export default handler;
            "#;
        let parsed_code = ParsedCode::try_from(refund_code.to_string()).unwrap();
        let fake_refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        //    and returns the response
        let invalid_request_amount_code = format!(
            r#"
            const handler = async () => {{
                const myHeaders = new Headers();
                myHeaders.append("X-JSTZ-AMOUNT", "{initial_caller_sf_balance}");
                return fetch(new Request("jstz://{fake_refund_sf}/", {{
                    headers: myHeaders
                }}));
            }};
            export default handler;
            "#
        );
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            ParsedCode::try_from(invalid_request_amount_code.to_string()).unwrap(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_before_source = Account::balance(host, &mut tx, &source).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .unwrap();
        assert!(result.status_code.is_server_error());
        tx.commit(host).unwrap();

        tx.begin();
        let balance_after_caller = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let balance_after_source = Account::balance(host, &mut tx, &source).unwrap();

        // // 4. Assert the refund is not propagated to the source
        assert_eq!(balance_before_caller, balance_after_caller);
        assert_eq!(balance_before_source, balance_after_source);
    }

    #[test]
    fn smart_function_refunds_succeeds() {
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        test_smart_function_refund(refund_code, refund_amount);
    }

    #[test]
    fn smart_function_refunds_succeeds_async() {
        let refund_amount = 1;
        let refund_code = format!(
            r#"
            const handler = async () => {{
                return new Response(null, {{
                    headers: {{ "X-JSTZ-TRANSFER": "{refund_amount}" }},
                }});
            }};
            export default handler;
            "#
        );
        test_smart_function_refund(refund_code, refund_amount);
    }

    fn test_smart_function_refund(refund_code: String, refund_amount: u64) {
        let source = Address::User(jstz_mock::account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_caller_sf_balance: u64 = 0;
        let initial_refund_sf_balance: u64 = 1;
        tx.begin();

        Account::add_balance(host, &mut tx, &source, initial_refund_sf_balance).unwrap();

        // 1. Deploy the smart function that refunds to the caller
        let parsed_code = ParsedCode::try_from(refund_code.to_string()).unwrap();
        let refund_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_refund_sf_balance,
        )
        .unwrap();

        // 2. deploy a smart function that calls the refund smart function
        let code = format!(
            r#"
            const handler = async () => {{
                const response = await fetch(new Request("jstz://{refund_sf}"));
                if (response.ok) {{
                    return new Response();
                }} else {{
                    return Response.error();
                }}  
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let caller_sf = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code.clone(),
            initial_caller_sf_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the caller smart function
        tx.begin();
        let balance_before = Account::balance(host, &mut tx, &caller_sf).unwrap();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &caller_sf).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .expect("run function expected");
        let balance_after = Account::balance(host, &mut tx, &caller_sf).unwrap();
        tx.commit(host).unwrap();

        // 4. Assert the refund from the refund smart function to the caller
        assert_eq!(balance_before + refund_amount, balance_after);

        // 5. Calling the transaction again results in an error and a tx rollback
        tx.begin();
        let transfer_amount = 1;
        Account::add_balance(host, &mut tx, &source, transfer_amount).unwrap();
        let balance_before = Account::balance(host, &mut tx, &source).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            transfer_amount.to_string().try_into().unwrap(),
        );
        let result = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            RunFunction {
                headers,
                ..run_function
            },
            fake_op_hash,
        )
        .unwrap();
        assert!(result.status_code.is_server_error());

        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_before, balance_after);
    }

    #[test]
    fn host_script_fa_withdraw_from_smart_function_succeeds() {
        let receiver = Address::User(jstz_mock::account1());
        let source = Address::User(jstz_mock::account2());
        let ticketer = jstz_mock::kt1_account1();
        let ticketer_string = ticketer.clone();
        let l1_proxy_contract = ticketer.clone();

        let ticket_id = 1234;
        let ticket_content = b"random ticket content".to_vec();
        let json_ticket_content = json!(&ticket_content);
        assert_eq!("[114,97,110,100,111,109,32,116,105,99,107,101,116,32,99,111,110,116,101,110,116]", format!("{}", json_ticket_content));
        let ticket =
            jstz_mock::parse_ticket(ticketer, 1, (ticket_id, Some(ticket_content)));
        let ticket_hash = ticket.hash().unwrap();
        let token_smart_function_initial_ticket_balance = 100;
        let withdraw_amount = 90;
        let mut jstz_mock_host = JstzMockHost::default();

        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();

        // 1. Deploy our "token contract"
        tx.begin();
        let token_contract_code = format!(
            r#"
                export default (request) => {{
                    const url = new URL(request.url)
                    if (url.pathname === "/withdraw") {{
                        const withdrawRequest = new Request("jstz://jstz/fa-withdraw", {{
                            method: "POST",
                            headers: {{
                                "Content-type": "application/json",
                            }},
                            body: JSON.stringify({{
                                amount: {withdraw_amount},
                                routingInfo: {{
                                    receiver: "{receiver}",
                                    proxyL1Contract: "{l1_proxy_contract}"
                                }},
                                ticketInfo: {{
                                    id: {ticket_id},
                                    content: {json_ticket_content},
                                    ticketer: "{ticketer_string}"
                                }}
                            }}),
                        }});
                        return SmartFunction.call(withdrawRequest);
                    }}
                    else {{
                        return Response.error();
                    }}

                }}
            "#,
        );
        let parsed_code = ParsedCode::try_from(token_contract_code.to_string()).unwrap();
        let token_smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        // 2. Add its ticket blance
        TicketTable::add(
            host,
            &mut tx,
            &token_smart_function,
            &ticket_hash,
            token_smart_function_initial_ticket_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the smart function
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/withdraw", &token_smart_function)
                .try_into()
                .unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = Blake2b::from(b"fake_op_hash".as_ref());
        smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash,
        )
        .expect("Fa withdraw expected");

        tx.commit(host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(1, outbox.len());
        tx.begin();
        let balance = TicketTable::get_balance(
            host,
            &mut tx,
            &Address::SmartFunction(token_smart_function),
            &ticket_hash,
        )
        .unwrap();
        assert_eq!(10, balance);

        // Trying a second fa withdraw should fail with insufficient funds
        tx.begin();
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let error = smart_function::run::execute(
            host,
            &mut tx,
            &source,
            run_function,
            fake_op_hash2,
        )
        .expect_err("Expected error");
        assert_eq!(
            "EvalError: TicketTableError: InsufficientFunds",
            error.to_string()
        );
    }

    #[test]
    fn call_smart_function_with_invalid_scheme_fails() {
        let kt1 = jstz_mock::kt1_account1();
        let self_address = jstz_mock::sf_account1();
        let mut jstz_rt = Runtime::new(10000).unwrap();
        let realm = jstz_rt.realm().clone();
        let context = jstz_rt.context();

        register_web_apis(&realm, context);

        let request = Request::from_http_request(
            http::Request::builder()
                .uri(format!("tezos://{kt1}"))
                .method("GET")
                .body(None)
                .unwrap(),
            context,
        )
        .unwrap();
        let request = JsNativeObject::new::<RequestClass>(request, context).unwrap();
        let operation_hash = OperationHash::from(b"abcdefghijslmnop".as_slice());
        let js_error =
            SmartFunction::call(&self_address, &request, operation_hash, context)
                .unwrap_err();
        assert_eq!("EvalError: InvalidScheme", js_error.to_string())
    }
}
