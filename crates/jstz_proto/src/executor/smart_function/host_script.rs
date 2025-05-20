use std::num::NonZeroU64;

use boa_engine::{
    object::ErasedObject, Context, JsError, JsNativeError, JsResult, JsValue,
};
use boa_gc::GcRefMut;
use http::Uri;
use jstz_api::http::{
    header::Headers,
    request::Request,
    response::{Response, ResponseClass},
};
use jstz_core::{native::JsNativeObject, runtime};

use crate::{
    context::account::{Account, Addressable},
    operation::RunFunction,
    runtime::v1::response_from_run_receipt,
};

use jstz_core::{
    host::HostRuntime,
    kv::{Storage, Transaction},
};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::Deserialize;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::storage::path::{OwnedPath, RefPath};

use crate::{
    error::Result,
    executor::{fa_withdraw::FaWithdraw, withdraw::Withdrawal},
    receipt::RunFunctionReceipt,
    Error,
};

pub const JSTZ_HOST: &str = "jstz";
const WITHDRAW_PATH: &str = "/withdraw";
const FA_WITHDRAW_PATH: &str = "/fa-withdraw";

pub const X_JSTZ_TRANSFER: &str = "X-JSTZ-TRANSFER";
pub const X_JSTZ_AMOUNT: &str = "X-JSTZ-AMOUNT";

pub(crate) struct HostScript;

impl HostScript {
    pub fn run(
        self_address: &impl Addressable,
        request: &mut GcRefMut<'_, ErasedObject, Request>,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let run = run_function_from_request(request, 1)?;
        let response = runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<Response> {
            // 1. Begin a new transaction
            tx.begin();
            // 2. Execute jstz host smart function
            let result = execute_without_ticketer(hrt, tx, self_address, run);

            // 3. Commit or rollback the transaction
            match result {
                Ok(run_receipt) => {
                    if run_receipt.status_code.is_success() {
                        tx.commit(hrt)?;
                    } else {
                        tx.rollback()?;
                    }
                    response_from_run_receipt(run_receipt, context)
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

fn run_function_from_request(
    request_deref: &mut GcRefMut<'_, ErasedObject, Request>,
    gas_limit: usize,
) -> JsResult<RunFunction> {
    let method = request_deref.method().clone();
    let uri = Uri::try_from(request_deref.url().clone().to_string()).map_err(|_| {
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

pub(crate) fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    ticketer: &ContractKt1Hash,
    source: &impl Addressable,
    run: RunFunction,
) -> Result<RunFunctionReceipt> {
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
            let receipt = RunFunctionReceipt {
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
            let receipt = RunFunctionReceipt {
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
) -> Result<RunFunctionReceipt> {
    let ticketer_path = OwnedPath::from(&RefPath::assert_from(b"/ticketer"));
    let ticketer: SmartFunctionHash =
        Storage::get(hrt, &ticketer_path)?.expect("ticketer should be set");
    execute(hrt, tx, &ticketer, source, run)
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use http::{HeaderName, HeaderValue};
    use jstz_api::http::{header::HeadersClass, request::RequestClass};
    use jstz_core::native::register_global_class;

    use super::*;

    use http::{header, HeaderMap, Method, Uri};
    use jstz_mock::host::JstzMockHost;
    use serde_json::json;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{
            account::{Account, Address},
            ticket_table::TicketTable,
        },
        executor::fa_withdraw::{RoutingInfo, TicketInfo},
    };

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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();

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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
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
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();

        execute(&mut host, &mut tx, &ticketer, &source, req)
            .expect("Withdraw should not fail");

        tx.begin();
        assert_eq!(0, Account::balance(&host, &mut tx, &source).unwrap());

        let level = host.run_level(|_| {});
        assert_eq!(1, host.outbox_at(level).len());
    }
}
