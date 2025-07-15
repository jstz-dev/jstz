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
    executor::smart_function::{
        host::execute_without_ticketer,
        run::{X_JSTZ_AMOUNT, X_JSTZ_TRANSFER},
    },
    operation::RunFunction,
};

use super::fetch_handler::response_from_run_receipt;

pub struct HostScript;

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
                        .with_message(format!("Invalid transfer amount: {e}")),
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
                            .with_message(format!("Transfer failed: {e}")),
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

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use http::{HeaderName, HeaderValue};
    use jstz_api::http::{header::HeadersClass, request::RequestClass};
    use jstz_core::native::register_global_class;

    use super::*;

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
                        .with_message(format!("Failed to create request: {e}")),
                )
            })?;

        // Set headers after building
        let headers_map = builder.headers_mut();
        for (key, value) in headers {
            headers_map.insert(
                HeaderName::from_str(&key).map_err(|e| {
                    JsError::from_native(
                        JsNativeError::error()
                            .with_message(format!("Invalid header name: {e}")),
                    )
                })?,
                HeaderValue::from_str(&value).map_err(|e| {
                    JsError::from_native(
                        JsNativeError::error()
                            .with_message(format!("Invalid header value: {e}")),
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
