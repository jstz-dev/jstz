use anyhow::{Context, Result};
use jstz_client::JstzClient;
use jstz_crypto::{
    public_key::PublicKey, public_key_hash::PublicKeyHash, secret_key::SecretKey,
};
use jstz_proto::context::account::Address;
use jstz_proto::operation::{Content, Operation, OracleResponse, SignedOperation};
use jstz_proto::receipt::{ReceiptContent, ReceiptResult};
use jstz_proto::runtime::v2::fetch::http::{convert_header_map, Body, Response};
use jstz_proto::runtime::v2::oracle::request::OracleRequest;
use log::{error, info};
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use reqwest::Method;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::task::AbortHandle;
use tokio_retry::{strategy::ExponentialBackoff, RetryIf};

#[allow(dead_code)]
pub struct DataProvider {
    abort_handle: AbortHandle,
}

impl DataProvider {
    #[allow(dead_code)]
    pub async fn spawn(
        public_key: PublicKey,
        secret_key: SecretKey,
        node_endpoint: String,
        mut relay_rx: Receiver<OracleRequest>,
    ) -> Result<Self> {
        let client = Client::builder()
            .user_agent("jstz-oracle-data-provider/0.1")
            .build()?;

        let abort_handle = {
            let task = tokio::spawn(async move {
                while let Ok(req) = relay_rx.recv().await {
                    if let Err(e) = handle_request(
                        &client,
                        &req,
                        &public_key,
                        &secret_key,
                        &node_endpoint,
                    )
                    .await
                    {
                        error!("Data provider error: {e:#}");
                    }
                }
            });
            task.abort_handle()
        };

        Ok(Self {
            abort_handle: abort_handle,
        })
    }
}

impl Drop for DataProvider {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

fn exponential_backoff(base: u64, attempts: usize) -> impl Iterator<Item = Duration> {
    ExponentialBackoff::from_millis(base)
        .factor(2)
        .max_delay(Duration::from_secs(8))
        .take(attempts)
}

async fn retry_async<F, Fut, T, E, C>(
    backoff: impl Iterator<Item = Duration>,
    mut op: F,
    should_retry: C,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    C: Fn(&E) -> bool + Copy,
{
    RetryIf::spawn(backoff, || op(), should_retry).await
}

async fn handle_request(
    client: &Client,
    oracle_req: &OracleRequest,
    public_key: &PublicKey,
    signing_key: &SecretKey,
    node_endpoint: &String,
) -> Result<()> {
    let response = get_oracle_response(client, oracle_req).await?;
    inject_oracle_response(oracle_req, public_key, signing_key, node_endpoint, response)
        .await?;

    Ok(())
}

fn is_transient_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<reqwest::Error>()
        .map(|re| re.is_timeout() || re.is_connect() || re.is_request())
        .unwrap_or(false)
}

async fn get_oracle_response(
    client: &Client,
    oracle_req: &OracleRequest,
) -> Result<Response> {
    let OracleRequest { request, .. } = oracle_req;

    // Convert to reqwest::RequestBuilder
    let method = Method::from_bytes(&request.method).context("invalid HTTP method")?;

    let attempt = || async {
        let mut builder = client.request(method.clone(), request.url.clone());

        // Headers
        let mut headers = ReqwestHeaderMap::new();
        for (name, value) in &request.headers {
            headers.append(
                HeaderName::from_bytes(name)?,
                HeaderValue::from_bytes(value)?,
            );
        }
        builder = builder.headers(headers);

        // Body
        if let Some(body) = request.body.clone() {
            builder = builder.body::<Vec<u8>>(body.into());
        }

        let resp = builder.send().await?;

        let status = resp.status().as_u16();
        let status_text = resp
            .status()
            .canonical_reason()
            .unwrap_or("Unknown")
            .to_string();

        // TODO: Update reqwest and simplify this
        let headers = convert_header_map(http::HeaderMap::from_iter(
            resp.headers().iter().map(|(name, value)| {
                (
                    http::HeaderName::from_bytes(name.as_str().as_bytes()).unwrap(),
                    http::HeaderValue::from_bytes(value.as_bytes()).unwrap(),
                )
            }),
        ));

        let body = Body::Vector(resp.bytes().await?.to_vec());

        Ok(Response {
            status,
            status_text,
            headers,
            body,
        })
    };

    // Retry only when it's safe and likely transient
    let should_retry = |e: &anyhow::Error| is_transient_error(e);

    // Execute
    let send_result =
        if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
            // 5 attempts: 100 ms → 3.2 s
            retry_async(exponential_backoff(100, 5), attempt, should_retry).await
        } else {
            // single attempt
            attempt().await
        };

    send_result.or_else(|e| Ok(bad_gateway_error_response(e.to_string().as_bytes())))
}

// MSDN reference: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/502
// The HTTP 502 Bad Gateway server error response status code
// indicates that a server was acting as a gateway or proxy
// and that it received an invalid response from the upstream server.
fn bad_gateway_error_response(body: &[u8]) -> Response {
    Response {
        status: 502,
        status_text: "Bad Gateway".into(),
        headers: vec![],
        body: body.to_vec().into(),
    }
}

async fn inject_oracle_response(
    oracle_req: &OracleRequest,
    public_key: &PublicKey,
    signing_key: &SecretKey,
    node_endpoint: &String,
    response: Response,
) -> Result<()> {
    let OracleRequest { id, .. } = oracle_req;

    // Build and sign operation
    let oracle_response = OracleResponse {
        request_id: id.clone(),
        response,
    };

    let jstz_client = JstzClient::new(node_endpoint.clone());

    let oracle_address = Address::User(PublicKeyHash::from(public_key));
    let should_retry = |e: &anyhow::Error| {
        if is_transient_error(e) {
            return true;
        }

        let msg = e.to_string();
        if let Some(pos) = msg.find("Status:") {
            if let Some(code_str) = msg[pos + 7..].trim().split_whitespace().next() {
                if let Ok(code) = code_str.parse::<u16>() {
                    return code == 429 || (500..600).contains(&code);
                }
            }
        }
        false
    };
    let nonce = RetryIf::spawn(
        ExponentialBackoff::from_millis(200)
            .factor(2)
            .max_delay(Duration::from_secs(5))
            .take(6), // total 6 tries
        || async {
            jstz_client
                .get_nonce(&oracle_address)
                .await
                .map_err(Into::into)
        },
        should_retry,
    )
    .await?;

    let op = Operation {
        public_key: public_key.clone(),
        nonce,
        content: Content::OracleResponse(oracle_response),
    };

    let op_hash = op.hash();

    let signed_op = SignedOperation::new(signing_key.sign(op_hash.clone())?, op);
    // Post operation to node
    RetryIf::spawn(
        ExponentialBackoff::from_millis(200)
            .factor(2)
            .max_delay(Duration::from_secs(5))
            .take(6),
        || async {
            jstz_client
                .post_operation(&signed_op)
                .await
                .map_err(Into::into)
        },
        should_retry,
    )
    .await?;
    let receipt = jstz_client.wait_for_operation_receipt(&op_hash).await?;

    match receipt.result {
        ReceiptResult::Success(ReceiptContent::OracleResponse(deploy)) => {
            info!("Oracle response injected for id={}", deploy.request_id);
            Ok(())
        }
        ReceiptResult::Success(_) => Err(anyhow::anyhow!(
            "Expected a `OracleResponse` receipt, but got something else."
        )),
        ReceiptResult::Failed(err) => Err(anyhow::anyhow!(
            "Failed to inject oracle response with error {err}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jstz_proto::runtime::v2::fetch::http::Body;
    use jstz_proto::runtime::v2::fetch::http::Request as HttpReq;
    use mockito::Matcher;
    use once_cell::sync::Lazy;
    use std::str::FromStr;
    use tokio::time::Duration;
    use url::Url;

    static CLIENT: Lazy<Client> = Lazy::new(|| {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("jstz-oracle-integ-test/0.1")
            .build()
            .expect("reqwest client")
    });

    fn oracle_req(method: &str, url: Url, body: Option<Body>) -> OracleRequest {
        OracleRequest {
            id: 99,
            caller: PublicKeyHash::from_str("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
                .unwrap(),
            gas_limit: 0,
            timeout: 0,
            request: HttpReq {
                method: method.into(),
                url,
                headers: vec![],
                body,
            },
        }
    }

    #[tokio::test]
    async fn fetches_example_dot_com() -> Result<()> {
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><h1>Example Domain</h1></html>")
            .create();

        let url = Url::parse(&format!("{}/", server.url()))?;
        let req = oracle_req("GET", url, None);

        let response = super::get_oracle_response(&CLIENT, &req).await?;
        let binding = response.body.to_vec();
        let html = std::str::from_utf8(&binding)?;
        assert!(
            html.contains("Example Domain"),
            "unexpected response body: {html}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn handles_gateway_proxy_errors() -> Result<()> {
        let url = Url::parse(&format!("{}/", "http://abc123"))?;
        let req = oracle_req("GET", url, None);

        let response = super::get_oracle_response(&CLIENT, &req).await?;
        assert_eq!(response.status, 502);
        assert_eq!(response.status_text, "Bad Gateway");
        // We do not compare body because the actual error in CI and local differ
        Ok(())
    }

    #[tokio::test]
    async fn echoes_post_body_via_httpbin() -> Result<()> {
        let mut server = mockito::Server::new_async().await;

        let payload = br#"{"msg":"hello world"}"#.to_vec();
        let _m = server
            .mock("POST", "/post")
            .match_body(Matcher::Exact(String::from_utf8(payload.clone())?))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data": "{\"msg\":\"hello world\"}"}"#)
            .create();

        let url = Url::parse(&format!("{}/post", server.url()))?;
        let req = oracle_req("POST", url, Some(Body::Vector(payload)));

        let response = super::get_oracle_response(&CLIENT, &req).await?;
        let binding = response.body.to_vec();
        let text = std::str::from_utf8(&binding)?;

        // httpbin returns the posted body in the `"data"` JSON field.
        assert!(
            text.contains(r#""data": "{\"msg\":\"hello world\"}""#),
            "response did not echo body: {text}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn injects_oracle_response_successfully() -> Result<()> {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;

        // Mock the nonce endpoint
        let mock_nonce = server
            .mock(
                "GET",
                "/accounts/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/nonce",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("0")
            .create();

        // Mock the post operation endpoint
        let mock_post_op = server.mock("POST", "/operations").with_status(200).create();

        // Mock the receipt endpoint
        let mock_receipt = server
            .mock("GET", "/operations/9b14cf6a10e07c8f4fb436a0e137e230a8fb5a2ea736316c9d428fa56d9c4414/receipt")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "hash": [160, 154, 126, 219, 223, 115, 53, 86, 77, 202, 57, 246, 177, 186, 154, 113, 31, 119, 80, 174, 115, 156, 171, 240, 255, 66, 118, 156, 97, 188, 60, 197],
                "result": {
                    "_type": "Success",
                    "inner": {
                        "_type": "OracleResponse",
                        "requestId": 99
                    }
                }
            }"#,
            )
            .create();

        // Create test data
        let public_key = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )?;

        let node_config = String::from(server.url().as_str());

        let oracle_req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let response = Response {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![],
            body: Body::Vector("test response data".into()),
        };

        // Call the function
        let result = inject_oracle_response(
            &oracle_req,
            &public_key,
            &secret_key,
            &node_config,
            response,
        )
        .await;

        // Verify the result
        assert!(
            result.is_ok(),
            "inject_oracle_response failed: {:?}",
            result.err()
        );

        // Verify all mocks were called
        mock_nonce.assert();
        mock_post_op.assert();
        mock_receipt.assert();

        Ok(())
    }

    #[tokio::test]
    async fn injects_oracle_response_with_failed_receipt() -> Result<()> {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;

        // Mock the nonce endpoint
        let mock_nonce = server
            .mock(
                "GET",
                "/accounts/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/nonce",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("0")
            .create();

        // Mock the post operation endpoint
        let mock_post_op = server.mock("POST", "/operations").with_status(200).create();

        // Mock the receipt endpoint with a failed result
        let mock_receipt = server
            .mock("GET", "/operations/9b14cf6a10e07c8f4fb436a0e137e230a8fb5a2ea736316c9d428fa56d9c4414/receipt")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "hash": [160, 154, 126, 219, 223, 115, 53, 86, 77, 202, 57, 246, 177, 186, 154, 113, 31, 119, 80, 174, 115, 156, 171, 240, 255, 66, 118, 156, 97, 188, 60, 197],
                "result": {
                    "_type": "Failed",
                    "inner": "Operation failed"
                }
            }"#,
            )
            .create();

        // Create test data
        let public_key = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )?;
        let node_config = String::from(server.url().as_str());

        let oracle_req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let response = Response {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![],
            body: Body::Vector("test response data".into()),
        };

        // Call the function
        let result = inject_oracle_response(
            &oracle_req,
            &public_key,
            &secret_key,
            &node_config,
            response,
        )
        .await;
        // Verify the result is an error
        assert!(result.is_err(), "Expected error for failed receipt");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to inject oracle response"));

        // Verify all mocks were called
        mock_nonce.assert();
        mock_post_op.assert();
        mock_receipt.assert();

        Ok(())
    }

    #[tokio::test]
    async fn retries_nonce_and_post_operation_then_succeeds() -> Result<()> {
        let mut server = mockito::Server::new_async().await;

        let nonce_path = "/accounts/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx/nonce";
        let _nonce_fail = server
            .mock("GET", nonce_path)
            .with_status(500)
            .expect(1)
            .create();
        let _nonce_ok = server
            .mock("GET", nonce_path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("0")
            .expect(1)
            .create();

        let _post_fail = server
            .mock("POST", "/operations")
            .with_status(500)
            .expect(1)
            .create();
        let _post_ok = server
            .mock("POST", "/operations")
            .with_status(200)
            .expect(1)
            .create();

        let _receipt_ok = server
            .mock("GET", "/operations/9b14cf6a10e07c8f4fb436a0e137e230a8fb5a2ea736316c9d428fa56d9c4414/receipt")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "hash": [160,154,126,219,223,115,53,86,77,202,57,246,177,186,154,113,31,119,80,174,115,156,171,240,255,66,118,156,97,188,60,197],
                "result": { "_type": "Success", "inner": { "_type": "OracleResponse", "requestId": 99 } }
            }"#)
            .expect(1)
            .create();

        let public_key = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )?;

        let node_cfg = String::from(server.url().as_str());

        let oracle_req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let fake_http_resp = Response {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: Body::Vector("test response data".into()),
        };

        inject_oracle_response(
            &oracle_req,
            &public_key,
            &secret_key,
            &node_cfg,
            fake_http_resp,
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn get_retries_on_timeout_then_succeeds() -> Result<()> {
        use once_cell::sync::Lazy;

        static FAST_CLIENT: Lazy<Client> = Lazy::new(|| {
            Client::builder()
                .timeout(Duration::from_millis(50)) // 50 ms total timeout
                .user_agent("jstz-oracle-integ-test/0.1")
                .build()
                .expect("reqwest client")
        });

        let mut server = mockito::Server::new_async().await;

        // 1st GET: delayed body ⇒ reqwest times out
        let _slow = server
            .mock("GET", "/data")
            .with_status(200)
            // send the whole body ("later") as one chunk,
            // but only after sleeping 200 ms  (> client timeout)
            .with_chunked_body(|mut writer| {
                std::thread::sleep(Duration::from_millis(200));
                writer.write_all(b"later").map(|_| ())
            })
            .expect(1)
            .create();

        // 2nd GET: immediate success
        let _fast = server
            .mock("GET", "/data")
            .with_status(200)
            .with_body("ok")
            .expect(1)
            .create();

        let url = Url::parse(&format!("{}/data", server.url()))?;
        let req = oracle_req("GET", url, None);

        let resp = super::get_oracle_response(&FAST_CLIENT, &req).await?;
        assert_eq!(resp.status, 200);
        assert_eq!(String::from_utf8(resp.body.to_vec())?, "ok");
        Ok(())
    }

    #[tokio::test]
    async fn post_is_not_retried_on_timeout() -> Result<()> {
        use once_cell::sync::Lazy;

        static FAST_CLIENT: Lazy<Client> = Lazy::new(|| {
            Client::builder()
                .timeout(Duration::from_millis(50))
                .user_agent("jstz-oracle-integ-test/0.1")
                .build()
                .expect("reqwest client")
        });

        let mut server = mockito::Server::new_async().await;

        // Only one slow POST; if retried, the mock server panics.
        let _slow = server
            .mock("POST", "/echo")
            .with_status(200)
            .with_chunked_body(|mut writer| {
                std::thread::sleep(Duration::from_millis(200));
                writer.write_all(b"ignored").map(|_| ())
            })
            .expect(1)
            .create();

        let url = Url::parse(&format!("{}/echo", server.url()))?;
        let payload = Body::Vector(br#"{"msg":"hello"}"#.to_vec());
        let req = oracle_req("POST", url, Some(payload));

        let resp = super::get_oracle_response(&FAST_CLIENT, &req).await?;
        assert_eq!(resp.status, 502);
        Ok(())
    }
}
