use boa_engine::{
    object::FunctionObjectBuilder, Context, JsArgs, JsError, JsNativeError, JsResult,
    JsValue, NativeFunction,
};
use jstz_api::http::{
    body::{Body, BodyWithType},
    header::Headers,
    request::{Request, RequestClass},
    response::{Response, ResponseBuilder, ResponseClass, ResponseOptions},
};
use jstz_core::{
    host::HostRuntime, kv::Transaction, native::JsNativeObject, runtime, Runtime,
};
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    context::account::{Account, Address, Addressable, ParsedCode},
    error::Result,
    executor::{
        smart_function::{
            host_script::HostScript,
            script::{register_http_api, Script},
        },
        JSTZ_HOST,
    },
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
    request_logger::{log_request_end, log_request_start},
    Error,
};

mod headers {
    use super::*;
    pub const REFERRER: &str = "Referer";

    pub(crate) fn test_and_set_referrer(
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
fn try_apply_to_value_or_promise<F1, F2>(
    value_or_promise: JsResult<JsValue>,
    on_fulfilled: F1,
    on_rejected: F2,
    context: &mut Context,
) -> JsResult<JsValue>
where
    F1: Fn(JsValue, &mut Context) -> JsResult<JsValue> + 'static,
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
                                    on_fulfilled(value, context)
                                },
                            )
                        })
                        .build(),
                    ),
                    Some(
                        FunctionObjectBuilder::new(context.realm(), unsafe {
                            NativeFunction::from_closure(
                                move |_, args, context| -> JsResult<JsValue> {
                                    let value = args.get_or_undefined(0).clone();
                                    let reason = JsError::from_opaque(value);
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
            None => on_fulfilled(value, context),
        },
        Err(err) => {
            on_rejected(context)?;
            Err(err)
        }
    }
}

fn runtime_and_request_from_run_operation(
    run_operation: RunFunction,
) -> Result<(Runtime, JsNativeObject<Request>)> {
    let RunFunction {
        uri,
        method,
        headers,
        body,
        gas_limit,
    } = run_operation;

    let mut rt = Runtime::new(gas_limit)?;
    register_http_api(&rt.realm().clone(), &mut rt);

    let mut http_request_builder = http::Request::builder().uri(uri).method(method);

    *http_request_builder
        .headers_mut()
        .ok_or(Error::InvalidHttpRequest)? = headers;

    let http_request = http_request_builder
        .body(body)
        .map_err(|_| Error::InvalidHttpRequest)?;
    let request = JsNativeObject::new::<RequestClass>(
        Request::from_http_request(http_request, &mut rt)?,
        &mut rt,
    )?;

    Ok((rt, request))
}

pub(crate) fn response_from_run_receipt(
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

/// skips the execution of the smart function for noop requests
pub const NOOP_PATH: &str = "/-/noop";

/// Handles the transfer (or a refund) (if any) and returns [`Ok(None)`] if the transfer
/// succeeded. If the transfer fails, it rolls back the transaction and returns an error response
/// as [`Ok(Some(err_response))`].
fn handle_transfer_or_rollback_and_return_response(
    headers: &mut Headers,
    source_address: &impl Addressable,
    dest_address: &impl Addressable,
    context: &mut Context,
) -> Result<Option<JsValue>> {
    if HostScript::handle_transfer(headers, source_address, dest_address).is_err() {
        // If the transfer fails, return an error response
        runtime::with_js_tx(|tx| tx.rollback())?;
        Ok(Some(
            JsNativeObject::new::<ResponseClass>(
                ResponseBuilder::error(context)?,
                context,
            )?
            .into(),
        ))
    } else {
        Ok(None)
    }
}

// Handles a fetch request to a smart function or a user address in the Jstz protocol.
pub(crate) fn fetch(
    source_address: &(impl Addressable + 'static),
    operation_hash: OperationHash,
    request: &JsNativeObject<Request>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let mut request_deref = request.deref_mut();
    if request_deref.url().scheme() != "jstz" {
        return Err(Error::InvalidScheme.into());
    }
    match request_deref.url().domain() {
        Some(JSTZ_HOST) => HostScript::run(source_address, &mut request_deref, context),
        Some(dest_address) => {
            let dest_address = Address::from_base58(dest_address).map_err(|_| {
                JsError::from_native(JsNativeError::error().with_message("Invalid host"))
            })?;

            runtime::with_js_tx(|tx| tx.begin());

            // 1. Handle the transfer operation in request headers
            if let Some(error) = handle_transfer_or_rollback_and_return_response(
                &mut request_deref.headers().deref_mut(),
                source_address,
                &dest_address,
                context,
            )? {
                // If the transfer fails, return an error response
                return Ok(error);
            }

            // 3. Handle request
            let is_noop = request_deref.url().path() == NOOP_PATH;
            match dest_address {
                Address::SmartFunction(dest_address) if !is_noop => {
                    // 5. Set the referrer of the request to the current smart function address
                    headers::test_and_set_referrer(&request_deref, source_address)?;

                    // 6. Load, init and run the smart function
                    let src_code =
                        runtime::with_js_hrt_and_tx(|hrt, tx| -> Result<ParsedCode> {
                            Ok(Account::function_code(hrt, tx, &dest_address)?.clone())
                        })?;

                    log_request_start(dest_address.clone(), operation_hash.to_string());
                    let response = Script::load_init_run(
                        &src_code,
                        dest_address.clone(),
                        operation_hash.clone(),
                        request.inner(),
                        context,
                    );

                    // TODO: avoid cloning
                    // https://linear.app/tezos/issue/JSTZ-331/avoid-cloning-for-address-in-proto
                    let source_address_clone = source_address.clone();
                    let dest_address_clone = dest_address.clone();
                    let response = try_apply_to_value_or_promise(
                        response,
                        move |value, context| -> JsResult<JsValue> {
                            match Response::try_from_js(&value) {
                                Ok(response) => {
                                    if let Some(error) =
                                        handle_transfer_or_rollback_and_return_response(
                                            &mut response.headers().deref_mut(),
                                            &dest_address_clone,
                                            &source_address_clone,
                                            context,
                                        )?
                                    {
                                        return Ok(error);
                                    }

                                    // If the smart function returns a valid response, commit the inner
                                    // transaction iff the response is ok
                                    runtime::with_js_hrt_and_tx(|hrt, tx| {
                                        if response.ok() {
                                            tx.commit(hrt)
                                        } else {
                                            tx.rollback()
                                        }
                                    })?;
                                }
                                _ => {
                                    // If the smart function doesn't return a valid response,
                                    // rollback the inner transaction (abort)
                                    runtime::with_js_tx(|tx| tx.rollback())?;
                                }
                            }
                            Ok(value)
                        },
                        |_context| Ok(runtime::with_js_tx(|tx| tx.rollback())?),
                        context,
                    );

                    log_request_end(dest_address.clone(), operation_hash.to_string());
                    response
                }
                _ => {
                    // Request is a noop request or to a user address
                    runtime::with_js_hrt_and_tx(|hrt, tx| tx.commit(hrt))?;

                    // Return a default response
                    let response =
                        Response::new(Default::default(), Default::default(), context)?;
                    JsNativeObject::new::<ResponseClass>(response, context)
                        .map(|obj| obj.inner().clone())
                }
            }
        }
        None => Err(JsError::from_native(
            JsNativeError::error().with_message("Invalid host"),
        ))?,
    }
}

fn run_toplevel_fetch(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt> {
    let gas_limit = run_operation.gas_limit;
    let (mut rt, request) = runtime_and_request_from_run_operation(run_operation)?;

    let result = {
        let rt = &mut rt;
        runtime::enter_js_host_context(hrt, tx, || {
            jstz_core::future::block_on(async move {
                let result = fetch(source_address, operation_hash, &request, rt)?;
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

    debug_msg!(hrt, "🚀 Smart function executed successfully with value: {:?} (in {:?} instructions)\n", result, gas_limit - rt.instructions_remaining());

    let response = Response::try_from_js(&result)?;
    let (http_parts, body) = Response::to_http_response(&response).into_parts();
    Ok(RunFunctionReceipt {
        body,
        status_code: http_parts.status,
        headers: http_parts.headers,
    })
}

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt> {
    run_toplevel_fetch(hrt, tx, source, run_operation, operation_hash)
}

#[cfg(test)]
mod test {
    use super::*;
    use http::{HeaderMap, Method};
    use jstz_core::kv::Transaction;
    use jstz_crypto::hash::Blake2b;
    use jstz_mock::host::JstzMockHost;

    use crate::{
        context::account::{Account, Address, ParsedCode},
        executor::smart_function::{
            self,
            host_script::{X_JSTZ_AMOUNT, X_JSTZ_TRANSFER},
        },
        operation::RunFunction,
    };

    #[test]
    fn transfer_xtz_to_and_from_smart_function_succeeds() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let transfer_amount = 3;
        let refund_amount = 2;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, transfer_amount)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, transfer_amount);
        tx.commit(host).unwrap();

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
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        let balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_before, 0);

        tx.commit(host).unwrap();

        // 2. Call the smart function
        tx.begin();
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
            &mut tx,
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
        tx.commit(host).unwrap();

        // 3. assert the transfer to the sf and refund to the source
        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(
            balance_after - balance_before,
            transfer_amount - refund_amount
        );
        assert_eq!(
            Account::balance(host, &mut tx, &source).unwrap(),
            refund_amount
        );

        // 4. transferring to the smart function should fail (source has insufficient funds)
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            fake_op_hash.clone(),
        )
        .unwrap();
        assert!(result.status_code.is_server_error());

        // 5. transferring from the smart function should fail with insufficient funds and the balance is rolled back
        let balance_before = Account::balance(host, &mut tx, &source).unwrap();
        // drain the balance of the smart function
        Account::set_balance(host, &mut tx, &smart_function, 0).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            X_JSTZ_TRANSFER,
            transfer_amount.to_string().try_into().unwrap(),
        );
        let result = execute(
            host,
            &mut tx,
            &source,
            RunFunction {
                headers,
                ..run_function
            },
            fake_op_hash.clone(),
        )
        .unwrap();
        assert!(result.status_code.is_server_error());

        // tx rolled back as smart function has insufficient funds
        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_after, balance_before);
    }

    #[test]
    fn transfer_xtz_to_smart_function_succeeds_with_noop_path() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, initial_balance);
        tx.commit(host).unwrap();

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
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        let balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_before, 0);

        tx.commit(host).unwrap();

        // transfer should happen with `/-/noop` path
        tx.begin();
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
        execute(host, &mut tx, &source, run_function.clone(), fake_op_hash)
            .expect("run function expected");
        tx.commit(host).unwrap();

        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_after - balance_before, initial_balance);
        assert_eq!(Account::balance(host, &mut tx, &source).unwrap(), 0);
    }

    #[test]
    fn transfer_xtz_to_user_succeeds() {
        let source = Address::User(jstz_mock::account1());
        let destination = Address::User(jstz_mock::account2());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        let source_balance = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(source_balance, initial_balance);
        tx.commit(host).unwrap();

        // 2. sending request to transfer from source to the destination
        tx.begin();
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
        let result = execute(host, &mut tx, &source, run_function.clone(), fake_op_hash);
        assert!(result.is_ok());

        tx.commit(host).unwrap();

        tx.begin();
        let balance_after = Account::balance(host, &mut tx, &source).unwrap();
        assert_eq!(balance_after, 0);
        assert_eq!(
            Account::balance(host, &mut tx, &destination).unwrap(),
            initial_balance
        );

        // 3. transferring again should fail
        let fake_op_hash2 = Blake2b::from(b"fake_op_hash2".as_ref());
        let result =
            execute(host, &mut tx, &source, run_function, fake_op_hash2).unwrap();
        assert!(result.status_code.is_server_error());
    }

    #[test]
    fn invalid_request_should_fails() {
        let source = Address::User(jstz_mock::account1());
        // 1. Deploy the smart function
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

        let code = r#"
             const handler = () => {{
                 return new Response();
             }};
             export default handler;
             "#;

        // 1. Deploy smart function
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();

        let sf_balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_before = Account::balance(host, &mut tx, &source).unwrap();
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
            &mut tx,
            &source,
            run_function.clone(),
            Blake2b::from(b"fake_op_hash".as_ref()),
        );
        let sf_balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_after = Account::balance(host, &mut tx, &source).unwrap();

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
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

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
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, initial_balance)
                .unwrap();

        let sf_balance_before = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_before = Account::balance(host, &mut tx, &source).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: Default::default(),
            body: None,
            gas_limit: 1000,
        };
        let result = execute(
            host,
            &mut tx,
            &source,
            run_function.clone(),
            Blake2b::from(b"fake_op_hash".as_ref()),
        );
        let sf_balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        let source_balance_after = Account::balance(host, &mut tx, &source).unwrap();

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
        let mut tx = Transaction::default();
        let initial_balance = 1;
        tx.begin();
        Account::add_balance(host, &mut tx, &source, initial_balance)
            .expect("add balance");
        tx.commit(host).unwrap();

        // 1. Deploy smart function
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let smart_function =
            smart_function::deploy(host, &mut tx, &source, parsed_code, 0).unwrap();

        tx.commit(host).unwrap();

        // Calling the smart function should error or return an error response
        tx.begin();
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
            &mut tx,
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
            Account::balance(host, &mut tx, &source).unwrap(),
            initial_balance
        );
        let balance_after = Account::balance(host, &mut tx, &smart_function).unwrap();
        assert_eq!(balance_after, 0);
    }
}
