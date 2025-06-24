use anyhow::{Context, Result};
use bytes::Bytes;
use jstz_client::JstzClient;
use jstz_crypto::{
    public_key::PublicKey, public_key_hash::PublicKeyHash, secret_key::SecretKey,
};
use jstz_node::config::JstzNodeConfig;
use jstz_proto::context::account::Address;
use jstz_proto::operation::{Content, Operation, OracleResponse, SignedOperation};
use jstz_proto::receipt::{ReceiptContent, ReceiptResult};
use jstz_proto::runtime::v2::oracle::request::OracleRequest;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use reqwest::Method;
use tokio::sync::broadcast::Receiver;

#[allow(dead_code)]
pub struct DataProvider {
    _task: tokio::task::JoinHandle<()>,
}

impl DataProvider {
    #[allow(dead_code)]
    pub async fn spawn(
        public_key: PublicKey,
        secret_key: SecretKey,
        node_endpoint: JstzNodeConfig,
        mut relay_rx: Receiver<OracleRequest>,
    ) -> Result<Self> {
        let client = Client::builder()
            .user_agent("jstz-oracle-data-provider/0.1")
            .build()?;

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
                    eprintln!("Data provider error: {e:#}");
                }
            }
        });

        Ok(Self { _task: task })
    }
}

async fn handle_request(
    client: &Client,
    oracle_req: &OracleRequest,
    public_key: &PublicKey,
    signing_key: &SecretKey,
    node_endpoint: &JstzNodeConfig,
) -> Result<()> {
    let resp_bytes = get_oracle_response(client, oracle_req).await?;
    inject_oracle_response(
        oracle_req,
        public_key,
        signing_key,
        node_endpoint,
        resp_bytes,
    )
    .await?;

    Ok(())
}

async fn get_oracle_response(
    client: &Client,
    oracle_req: &OracleRequest,
) -> Result<Bytes> {
    let OracleRequest {
        id: _,
        request,
        caller: _,
        gas_limit: _,
        timeout: _,
    } = oracle_req;

    // Convert to reqwest::RequestBuilder
    let method = Method::from_bytes(&request.method).context("invalid HTTP method")?;
    let mut builder = client.request(method, request.url.clone());

    // Headers
    let mut headers = HeaderMap::new();
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
    let resp_bytes = builder.send().await?.bytes().await?;

    Ok(resp_bytes)
}

async fn inject_oracle_response(
    oracle_req: &OracleRequest,
    public_key: &PublicKey,
    signing_key: &SecretKey,
    node_endpoint: &JstzNodeConfig,
    resp_bytes: Bytes,
) -> Result<()> {
    let OracleRequest {
        id,
        request: _,
        caller: _,
        gas_limit: _,
        timeout: _,
    } = oracle_req;

    // Build and sign operation
    let oracle_response = OracleResponse {
        request_id: id.clone(),
        response: resp_bytes.to_vec(),
    };

    let jstz_client = JstzClient::new(node_endpoint.endpoint.to_string());

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
            eprintln!("Oracle response injected for id={}", deploy.request_id);
            Ok(())
        }
        ReceiptResult::Success(_) => Err(anyhow::anyhow!(
            "Expected a `OracleResponse` receipt, but got something else."
        )),
        ReceiptResult::Failed(err) => Err(anyhow::anyhow!(
            "Failed to inject oracle response with error {err:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jstz_node::config::{JstzNodeConfig, KeyPair};
    use jstz_node::RunMode;
    use jstz_proto::runtime::v2::fetch::http::Body;
    use jstz_proto::runtime::v2::fetch::http::Request as HttpReq;
    use octez::r#async::endpoint::Endpoint;
    use once_cell::sync::Lazy;
    use std::path::PathBuf;
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
        let req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let bytes = super::get_oracle_response(&CLIENT, &req).await?;
        let html = std::str::from_utf8(&bytes)?;
        assert!(
            html.contains("Example Domain"),
            "unexpected response body: {html}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn echoes_post_body_via_httpbin() -> Result<()> {
        let payload = br#"{"msg":"hello world"}"#.to_vec();
        let req = oracle_req(
            "POST",
            Url::parse("https://httpbin.org/post")?,
            Some(Body::Vector(payload.clone())),
        );

        let bytes = super::get_oracle_response(&CLIENT, &req).await?;
        let text = std::str::from_utf8(&bytes)?;

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
            .mock("GET", "/operations/a09a7debdf7335564dca39f6b1ba9a711f7750ae7396cabf0ff42769c61bc3c5/receipt")
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

        eprintln!("Mock nonce: {}", mock_nonce);

        // Create test data
        let public_key = PublicKey::from_base58(
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        )?;

        eprintln!("Starting node config");
        let node_config = JstzNodeConfig::new(
            &Endpoint::from_str(server.url().as_str())?,
            &Endpoint::from_str(server.url().as_str())?,
            &PathBuf::from("/tmp/preimages"),
            &PathBuf::from("/tmp/kernel.log"),
            KeyPair::default(),
            RunMode::Default,
            1000,
            &PathBuf::from("/tmp/debug.log"),
        );

        let oracle_req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let response_bytes = Bytes::from("test response data");

        // Call the function
        let result = inject_oracle_response(
            &oracle_req,
            &public_key,
            &secret_key,
            &node_config,
            response_bytes,
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
            .mock("GET", "/operations/a09a7debdf7335564dca39f6b1ba9a711f7750ae7396cabf0ff42769c61bc3c5/receipt")
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
        let node_config = JstzNodeConfig::new(
            &Endpoint::from_str(server.url().as_str())?,
            &Endpoint::from_str(server.url().as_str())?,
            &PathBuf::from("/tmp/preimages"),
            &PathBuf::from("/tmp/kernel.log"),
            KeyPair::default(),
            RunMode::Default,
            1000,
            &PathBuf::from("/tmp/debug.log"),
        );

        let oracle_req = oracle_req("GET", Url::parse("https://example.com")?, None);
        let response_bytes = Bytes::from("test response data");

        // Call the function
        let result = inject_oracle_response(
            &oracle_req,
            &public_key,
            &secret_key,
            &node_config,
            response_bytes,
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
