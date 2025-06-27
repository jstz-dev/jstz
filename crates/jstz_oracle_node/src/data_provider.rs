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
use tokio::sync::broadcast::Receiver;
use tokio::task::AbortHandle;

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

async fn get_oracle_response(
    client: &Client,
    oracle_req: &OracleRequest,
) -> Result<Response> {
    let OracleRequest { request, .. } = oracle_req;

    // Convert to reqwest::RequestBuilder
    let method = Method::from_bytes(&request.method).context("invalid HTTP method")?;
    let mut builder = client.request(method, request.url.clone());

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

    // Execute
    let response = builder.send().await?;
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    // TODO: Update reqwest and simplify this
    let headers = convert_header_map(http::HeaderMap::from_iter(
        response.headers().iter().map(|(name, value)| {
            (
                http::HeaderName::from_bytes(name.as_str().as_bytes()).unwrap(),
                http::HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            )
        }),
    ));

    let body_bytes = response.bytes().await?;
    let body = Body::Vector(body_bytes.to_vec());

    Ok(Response {
        status,
        status_text,
        headers,
        body,
    })
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
    let nonce = jstz_client.get_nonce(&oracle_address).await?;

    let op = Operation {
        public_key: public_key.clone(),
        nonce,
        content: Content::OracleResponse(oracle_response),
    };

    let op_hash = op.hash();

    let signed_op = SignedOperation::new(signing_key.sign(op_hash.clone())?, op);

    // Post operation to node
    jstz_client.post_operation(&signed_op).await?;
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
<<<<<<< HEAD
    use mockito::{Matcher, Mock};
    use octez::r#async::endpoint::Endpoint;
=======
>>>>>>> 08a86178 (feat(oracle): connect relay and data provider, integrate in jstz node)
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
                "/accounts/tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nonce",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("0")
            .create();

        // Mock the post operation endpoint
        let mock_post_op = server.mock("POST", "/operations").with_status(200).create();

        // Mock the receipt endpoint
        let mock_receipt = server
            .mock("GET", "/operations/e5f9460ca912defddcffab260339e25eeb1525725385ba1b9d9dd0b2c9dbdbb4/receipt")
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
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
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
                "/accounts/tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU/nonce",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("0")
            .create();

        // Mock the post operation endpoint
        let mock_post_op = server.mock("POST", "/operations").with_status(200).create();

        // Mock the receipt endpoint with a failed result
        let mock_receipt = server
            .mock("GET", "/operations/e5f9460ca912defddcffab260339e25eeb1525725385ba1b9d9dd0b2c9dbdbb4/receipt")
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
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
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
}
