use std::ops::Deref;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, ErasedObject, ObjectInitializer},
    property::Attribute,
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};

use jstz_api::http::request::Request;
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, native::JsNativeObject, runtime,
    value::IntoJs,
};

use crate::{
    context::{
        new_account::NewAddress,
        new_account::{Account, Amount, ParsedCode},
    },
    executor::{
        smart_function::{headers, HostScript, Script},
        JSTZ_HOST,
    },
    operation::OperationHash,
    Error, Result,
};

use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

#[derive(JsData)]
pub struct TraceData {
    pub address: NewAddress,
    pub operation_hash: OperationHash,
}

impl Finalize for TraceData {}

unsafe impl Trace for TraceData {
    empty_trace!();
}

#[derive(JsData)]
struct SmartFunction {
    address: NewAddress,
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
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        function_code: ParsedCode,
        initial_balance: Amount,
    ) -> Result<String> {
        let balance = Account::balance(hrt, tx, &self.address)?;
        if balance < initial_balance {
            return Err(Error::BalanceOverflow);
        }

        // 2. Deploy the smart function
        let address =
            Script::deploy(hrt, tx, &self.address, function_code, initial_balance)?;

        // 3. Increment nonce of current account
        let nonce = Account::nonce(hrt, tx, &self.address)?;
        nonce.increment();

        // 4. Transfer the balance to the associated account
        Account::transfer(hrt, tx, &self.address, &address, initial_balance)?;

        Ok(address.to_string())
    }

    // Invariant: The function should always be called within a js_host_context
    fn call(
        self_address: &NewAddress,
        request: &JsNativeObject<Request>,
        operation_hash: OperationHash,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. Get address from request
        let mut request_deref = request.deref_mut();
        match request_deref.url().domain() {
            Some(JSTZ_HOST) => HostScript::run(self_address, &mut request_deref, context),
            Some(address) => {
                let address = NewAddress::from_base58(address).map_err(|_| {
                    JsError::from_native(
                        JsNativeError::error().with_message("Invalid host"),
                    )
                })?;
                // 2. Set the referer of the request to the current smart function address
                headers::test_and_set_referrer(&request_deref, self_address)?;

                // 3. Load, init and run!
                Script::load_init_run(address, operation_hash, request.inner(), context)
            }
            None => Err(JsError::from_native(
                JsNativeError::error().with_message("Invalid host"),
            ))?,
        }
    }
}

pub struct SmartFunctionApi {
    pub address: NewAddress,
}

impl SmartFunctionApi {
    const NAME: &'static str = "SmartFunction";

    fn fetch(
        address: &NewAddress,
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
                    smart_function.create(
                        hrt.deref(),
                        tx,
                        parsed_code,
                        initial_balance as Amount,
                    )
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

    use crate::{
        context::{
            new_account::NewAddress,
            new_account::{Account, ParsedCode},
            ticket_table::TicketTable,
        },
        executor::smart_function::{self, register_web_apis, Script},
        operation::RunFunction,
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

        // TODO: Use sf address instead
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        let self_address = NewAddress::SmartFunction(
            SmartFunctionHash::digest(b"random bytes").unwrap(),
        );
        let amount = 100;

        let operation_hash = Blake2b::from(b"operation_hash".as_ref());
        let receiver =
            NewAddress::User(PublicKeyHash::digest(b"receiver address").unwrap());
        let http_request = http::Request::builder()
            .method(Method::POST)
            .uri("tezos://jstz/withdraw")
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
        let source = NewAddress::User(jstz_mock::account1());
        let code = r#"
        export default (request) => {
            const withdrawRequest = new Request("tezos://jstz/withdraw", {
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
        let smart_function = crate::executor::smart_function::Script::deploy(
            host,
            &mut tx,
            &source,
            parsed_code,
            5,
        )
        .unwrap();
        tx.commit(host).unwrap();

        tx.begin();
        let run_function = RunFunction {
            uri: format!("tezos://{}/", smart_function).try_into().unwrap(),
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
    fn host_script_fa_withdraw_from_smart_function_succeeds() {
        let receiver = NewAddress::User(jstz_mock::account1());
        let source = NewAddress::User(jstz_mock::account2());
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
        let token_smart_function_intial_ticket_balance = 100;
        let withdraw_amount = 90;
        let mut jstz_mock_hosh = JstzMockHost::default();

        let host = jstz_mock_hosh.rt();
        let mut tx = Transaction::default();

        // 1. Deploy our "token contract"
        tx.begin();
        let token_contract_code = format!(
            r#"
                export default (request) => {{
                    const url = new URL(request.url)
                    if (url.pathname === "/withdraw") {{
                        const withdrawRequest = new Request("tezos://jstz/fa-withdraw", {{
                            method: "POST",
                            headers: {{
                                "Content-type": "application/json",
                            }},
                            body: JSON.stringify({{
                                amount: {withdraw_amount},
                                routing_info: {{
                                    receiver: "{receiver}",
                                    proxy_l1_contract: "{l1_proxy_contract}"
                                }},
                                ticket_info: {{
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
            Script::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        // 2. Add its ticket blance
        TicketTable::add(
            host,
            &mut tx,
            &token_smart_function,
            &ticket_hash,
            token_smart_function_intial_ticket_balance,
        )
        .unwrap();
        tx.commit(host).unwrap();

        // 3. Call the smart function
        tx.begin();
        let run_function = RunFunction {
            uri: format!("tezos://{}/withdraw", &token_smart_function)
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
        let balance =
            TicketTable::get_balance(host, &mut tx, &token_smart_function, &ticket_hash)
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
}
