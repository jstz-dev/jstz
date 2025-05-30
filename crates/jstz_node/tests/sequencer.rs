use axum::http::{HeaderMap, Method, StatusCode, Uri};
use jstz_crypto::{
    public_key::PublicKey,
    signature::Signature,
    smart_function_hash::{Kt1Hash, SmartFunctionHash},
};
use jstz_proto::{
    context::account::Nonce,
    operation::{Content, DeployFunction, Operation, RunFunction, SignedOperation},
    receipt::{
        DeployFunctionReceipt, Receipt, ReceiptContent, ReceiptResult, RunFunctionReceipt,
    },
    runtime::ParsedCode,
};
use octez::unused_port;
use reqwest::Client;
use std::process::{Child, Command};
use tempfile::{NamedTempFile, TempDir};
use tezos_crypto_rs::hash::{Ed25519Signature, PublicKeyEd25519};

struct ChildWrapper(Child);

impl Drop for ChildWrapper {
    fn drop(&mut self) {
        if let Err(e) = self.0.kill() {
            println!("Could not kill child process: {}", e)
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn run_sequencer() {
    let tmp_dir = TempDir::new().unwrap();
    let log_file = NamedTempFile::new().unwrap();
    let port = unused_port();
    let base_uri = format!("http://127.0.0.1:{port}");

    let bin_path = assert_cmd::cargo::cargo_bin("jstz-node");
    let _c = ChildWrapper(
        Command::new(bin_path)
            .args([
                "run",
                "--port",
                &port.to_string(),
                "--preimages-dir",
                tmp_dir.path().to_str().unwrap(),
                "--kernel-log-path",
                log_file.path().to_str().unwrap(),
                "--mode",
                "sequencer",
            ])
            .spawn()
            .unwrap(),
    );

    let client = Client::new();

    check_mode(&client, &base_uri).await;
    deploy_function(&client, &base_uri).await;
    call_function(&client, &base_uri).await;
}

async fn check_mode(client: &Client, base_uri: &str) {
    let res = jstz_utils::poll(10, 500, || async {
        client.get(format!("{base_uri}/mode")).send().await.ok()
    })
    .await
    .expect("should get response")
    .text()
    .await
    .expect("should get text body");

    assert_eq!(res, "\"sequencer\"");
}

fn raw_operation(nonce: u64, content: Content) -> Operation {
    Operation {
        public_key: PublicKey::Ed25519(
            PublicKeyEd25519::from_base58_check(
                "edpkuXD2CqRpWoTT8p4exrMPQYR2NqsYH3jTMeJMijHdgQqkMkzvnz",
            )
            .unwrap()
            .into(),
        ),
        nonce: Nonce(nonce),
        content,
    }
}

fn signed_operation(sig_str: &str, raw_operation: Operation) -> SignedOperation {
    SignedOperation::new(
        Signature::Ed25519(Ed25519Signature::from_base58_check(sig_str).unwrap().into()),
        raw_operation,
    )
}

async fn submit_operation(
    client: &Client,
    base_uri: &str,
    operation: Operation,
    expected_hash: &str,
    sig_str: &str,
) -> Receipt {
    let hash = client
        .post(format!("{base_uri}/operations/hash"))
        .body(serde_json::to_string(&operation).unwrap())
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap()
        .json::<String>()
        .await
        .unwrap();

    assert_eq!(&hash, expected_hash);

    let signed_deploy_op = signed_operation(sig_str, operation);
    assert_eq!(
        client
            .post(format!("{base_uri}/operations"))
            .body(serde_json::to_string(&signed_deploy_op).unwrap())
            .header("content-type", "application/json")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    jstz_utils::poll(10, 500, || async {
        match client
            .get(format!("{base_uri}/operations/{hash}/receipt"))
            .send()
            .await
            .ok()
        {
            Some(r) if r.status() != 404 => Some(r),
            _ => None,
        }
    })
    .await
    .expect("should get response")
    .json::<Receipt>()
    .await
    .expect("should get receipt")
}

async fn deploy_function(client: &Client, base_uri: &str) {
    let deploy_op = raw_operation(0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from(format!("const handler = async () => {{ const s = \"{}\"; return new Response(\"this is a big function\"); }}; export default handler;\n", "a".repeat(8000))).unwrap(), account_credit: 0}));

    let receipt = submit_operation(client, base_uri, deploy_op, "bcab63dea88398b20ac20616d0fdcbd2d3042fd88f762a01a49f17ef082a511c", "edsigu5pnMWyk1EZm2bDpJivHEKC5XVX14RZdC9it5UCZMEUhKbC67LKTnMSxn39d1Sv2Vx6DK71LzpZ3MVDAGpNhPxvUxgNfe3").await;

    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::DeployFunction(
            DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
        )) if addr.to_base58_check() == "KT19TpJrE2ErutMMYqttsxmJQRNBHJpmYNsT"
    ));
}

async fn call_function(client: &Client, base_uri: &str) {
    let call_op = raw_operation(
        1,
        Content::RunFunction(RunFunction {
            uri: Uri::from_static("jstz://KT19TpJrE2ErutMMYqttsxmJQRNBHJpmYNsT/"),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 550000,
        }),
    );

    let receipt = submit_operation(client, base_uri, call_op, "617a081f5886482776c7d5c41f5f866a105974392162371af4f40a4524051d1c", "edsigte57eB4EtBsobBReepyvQHDYYvD1eLuR8ufvChyshUgLZYYed6ouTpVM4D8SKBB8hELPFoCN75V2tiXENJRpYUBibpZXnG").await;

    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
            body,
            status_code: StatusCode::OK,
            headers: _
        })) if &String::from_utf8(body.clone().unwrap()).unwrap() == "this is a big function"
    ));
}
