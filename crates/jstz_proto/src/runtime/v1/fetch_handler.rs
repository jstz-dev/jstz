use boa_engine::{
    object::FunctionObjectBuilder, Context, JsArgs, JsError, JsNativeError, JsResult,
    JsValue, NativeFunction,
};
use jstz_api::http::{
    body::{Body, BodyWithType, HttpBody},
    header::Headers,
    request::{Request, RequestClass},
    response::{Response, ResponseBuilder, ResponseClass, ResponseOptions},
};
use jstz_core::{native::JsNativeObject, runtime, Runtime};
use std::ops::Deref;

use crate::{
    context::account::{Account, Address, Addressable},
    error::{self, Result},
    executor::smart_function::{JSTZ_HOST, NOOP_PATH},
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
    request_logger::{log_request_end, log_request_start},
    Error,
};

use super::{
    api::{ProtocolApi, WebApi},
    host_script::HostScript,
    script::{ParsedCode, Script},
};

mod headers {

    use boa_engine::JsNativeError;
    use jstz_api::http::request::Request;

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

/// Handles a fetch request to a smart function or a user address in the Jstz protocol.
pub fn fetch(
    source_address: &(impl Addressable + 'static),
    operation_hash: OperationHash,
    request: &JsNativeObject<Request>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let mut request_deref = request.deref_mut();
    if request_deref.url().scheme() != "jstz" {
        return Err(error::Error::InvalidScheme.into());
    }
    match request_deref.url().domain() {
        Some(JSTZ_HOST) => HostScript::route(source_address, &mut request_deref, context),
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
                            Ok(ParsedCode(
                                Account::function_code(hrt, tx, &dest_address)?
                                    .deref()
                                    .to_string(),
                            ))
                        })?;

                    log_request_start(dest_address.clone(), operation_hash.to_string());
                    let response = Script::load_init_run(
                        &src_code,
                        ProtocolApi {
                            operation_hash: operation_hash.clone(),
                            address: dest_address.clone(),
                        },
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

pub fn runtime_and_request_from_run_operation(
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
    rt.realm().clone().register_api(WebApi, &mut rt);

    let http_request = create_http_request(uri, method, headers, body)?;
    let request = JsNativeObject::new::<RequestClass>(
        Request::from_http_request(http_request, &mut rt)?,
        &mut rt,
    )?;

    Ok((rt, request))
}

pub fn response_from_run_receipt(
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

#[cfg(test)]
mod test {
    use crate::{
        context::account::{Account, Address},
        executor::smart_function,
        operation::{OperationHash, RunFunction},
        runtime::ParsedCode,
    };
    use http::{HeaderMap, Method};
    use jstz_api::http::request::{Request, RequestClass};
    use jstz_core::{kv::Transaction, native::JsNativeObject, runtime, Runtime};
    use jstz_mock::{account1, account2, host::JstzMockHost};
    use tezos_smart_rollup_mock::MockHost;

    use crate::runtime::v1::api::WebApi;

    #[test]
    fn call_smart_function_with_invalid_scheme_fails() {
        let kt1 = jstz_mock::kt1_account1();
        let self_address = jstz_mock::sf_account1();
        let mut jstz_rt = Runtime::new(10000).unwrap();
        let realm = jstz_rt.realm().clone();
        let context = jstz_rt.context();

        realm.register_api(WebApi, context);

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

        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let js_error = runtime::enter_js_host_context(&mut host, &mut tx, || {
            super::fetch(&self_address, operation_hash, &request, context).unwrap_err()
        });
        assert_eq!("EvalError: InvalidScheme", js_error.to_string())
    }

    #[test]
    fn host_script_balance_endpoint_returns_correct_balance() {
        let source = Address::User(account1());
        let test_account = Address::User(account2());
        let mut jstz_mock_host = JstzMockHost::default();
        let host = jstz_mock_host.rt();
        let mut tx = Transaction::default();
        let expected_balance_self = 47;
        let expected_balance_test_account = 147;

        // Set up initial balance
        tx.begin();
        Account::add_balance(host, &mut tx, &test_account, expected_balance_test_account)
            .unwrap();
        Account::add_balance(host, &mut tx, &source, expected_balance_self).unwrap();
        tx.commit(host).unwrap();

        // Deploy a smart function that checks balances
        let code = format!(
            r#"
            const handler = async () => {{
                // Check balance of specific address
                const response1 = await fetch(new Request("jstz://jstz/balances/{test_account}"));
                const balance1 = await response1.json();
                if (balance1 !== {expected_balance_test_account}) {{
                    throw new Error(`Expected balance {expected_balance_test_account}, got ${{balance1}}`);
                }}

                // Check self balance
                const response2 = await fetch(new Request("jstz://jstz/balances/self"));
                const balance2 = await response2.json();
                if (balance2 !== {expected_balance_self}) {{
                    throw new Error(`Expected self balance {expected_balance_self}, got ${{balance2}}`);
                }}

                // Check balance of source address
                const response3 = await fetch(new Request("jstz://jstz/balances/{source}"));
                const balance3 = await response3.json();
                if (balance3 !== 0) {{
                    throw new Error(`Expected balance 0, got ${{balance3}}`);
                }}

                // Check invalid address
                try {{
                    const response4 = await fetch(new Request("jstz://jstz/balances/invalid_address"));
                    const balance4 = await response4.json();
                    throw new Error("Expected error since the address is invalid, but got response");
                }} catch (error) {{
                    if (!error.message.includes("Invalid address")) {{
                        throw new Error(`Expected "Invalid address" error, got: ${{error.message}}`);
                    }}
                }}

                return new Response("OK");
            }};
            export default handler;
            "#
        );
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let smart_function = smart_function::deploy(
            host,
            &mut tx,
            &source,
            parsed_code,
            expected_balance_self,
        )
        .unwrap();

        // Call the smart function
        let run_function = RunFunction {
            uri: format!("jstz://{}/", &smart_function).try_into().unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 1000,
        };
        let fake_op_hash = OperationHash::from(b"balanceop".as_ref());
        smart_function::run::execute(host, &mut tx, &source, run_function, fake_op_hash)
            .expect("run function expected");
        tx.commit(host).unwrap();
    }
}
