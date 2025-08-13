use axum::http::{HeaderMap, Method, StatusCode, Uri};
use bytes::Bytes;
use jstz_crypto::{
    public_key_hash::PublicKeyHash,
    smart_function_hash::{Kt1Hash, SmartFunctionHash},
};
use jstz_kernel::inbox::RollupType;
use jstz_node::sequencer::{inbox::api::BlockResponse, runtime::JSTZ_ROLLUP_ADDRESS};
use jstz_proto::{
    context::account::{Address, Nonce},
    executor::fa_deposit::FaDepositReceipt,
    operation::{
        internal::InboxId, Content, DeployFunction, Operation, RunFunction,
        SignedOperation,
    },
    receipt::{
        DeployFunctionReceipt, DepositReceipt, Receipt, ReceiptContent, ReceiptResult,
        RunFunctionReceipt,
    },
    runtime::ParsedCode,
    HttpBody,
};
use jstz_utils::{test_util::alice_keys, KeyPair};
use octez::unused_port;
use reqwest::Client;
use std::{
    io::Write,
    process::{Child, Command},
};
use tempfile::{NamedTempFile, TempDir};
use tokio_stream::StreamExt;

use futures_util::stream;
use inbox_utils::*;
use jstz_core::BinEncodable;
use std::convert::Infallible;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::{ExternalMessageFrame, InboxMessage};
use tezos_smart_rollup::types::SmartRollupAddress;
use tokio::{
    task::{self, JoinHandle},
    time::{sleep, Duration},
};
use warp::{hyper::Body, Filter};

struct ChildWrapper(Child);

impl Drop for ChildWrapper {
    fn drop(&mut self) {
        if let Err(e) = self.0.kill() {
            println!("Could not kill child process: {e}")
        }
    }
}

const DEFAULT_ROLLUP_NODE_RPC: &str = "127.0.0.1:8932";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_sequencer() {
    let tmp_dir = TempDir::new().unwrap();
    let log_file = NamedTempFile::new().unwrap();
    let mut injector_file = NamedTempFile::new().unwrap();
    injector_file
        .write_all(
            br#"{
            "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
            "secret_key": "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
}"#,
        )
        .unwrap();
    injector_file.flush().unwrap();
    let port = unused_port();
    let base_uri = format!("http://127.0.0.1:{port}");
    let _rollup_rpc = make_mock_rollup_rpc_server(DEFAULT_ROLLUP_NODE_RPC.to_string());

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
                "--injector-key-file",
                injector_file.path().to_str().unwrap(),
            ])
            .spawn()
            .unwrap(),
    );

    let client = Client::new();

    check_mode(&client, &base_uri).await;
    check_worker_health(&client, &base_uri).await;
    deploy_function(&client, &base_uri).await;
    call_function_and_stream_logs(&base_uri).await;
    check_inbox_op(&client, &base_uri).await;
    check_worker_health(&client, &base_uri).await;
}

async fn check_mode(client: &Client, base_uri: &str) {
    let res = jstz_utils::poll(15, 500, || async {
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
        public_key: jstz_mock::pk1(),
        nonce: Nonce(nonce),
        content,
    }
}

fn signed_operation(raw_operation: Operation) -> SignedOperation {
    SignedOperation::new(
        jstz_mock::sk1().sign(raw_operation.hash()).unwrap(),
        raw_operation,
    )
}

async fn submit_operation(
    client: &Client,
    base_uri: &str,
    operation: Operation,
    expected_hash: &str,
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

    let signed_deploy_op = signed_operation(operation);
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

    poll_receipt(client, base_uri, &hash).await
}

async fn poll_receipt(client: &Client, base_uri: &str, hash: &str) -> Receipt {
    let uri = format!("{base_uri}/operations/{hash}/receipt");
    jstz_utils::poll(10, 500, || async {
        match client.get(&uri).send().await.ok() {
            Some(r) if r.status() != 404 => Some(r),
            _ => None,
        }
    })
    .await
    .unwrap_or_else(|| panic!("should get response from {uri}"))
    .json::<Receipt>()
    .await
    .unwrap_or_else(|e| panic!("should get receipt from {uri} but got error {e:?}"))
}

async fn deploy_function(client: &Client, base_uri: &str) {
    let deploy_op = raw_operation(0, Content::DeployFunction(DeployFunction {function_code: ParsedCode::try_from(format!("const handler = async () => {{ const s = \"{}\"; console.log(\"debug message here\"); return new Response(\"this is a big function\"); }}; export default handler;\n", "a".repeat(8000))).unwrap(), account_credit: 0}));

    let receipt = submit_operation(
        client,
        base_uri,
        deploy_op,
        "931008aa770c77c72df2e7417832773030d65e113faa8836637b953932736fd3",
    )
    .await;

    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::DeployFunction(
            DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
        )) if addr.to_base58_check() == "KT1Lk9dy6cfWTQdB89rFK6P3tPDmfGdRmHee"
    ));
}

async fn call_function(client: &Client, base_uri: &str) {
    let call_op = raw_operation(
        1,
        Content::RunFunction(RunFunction {
            uri: Uri::from_static("jstz://KT1Lk9dy6cfWTQdB89rFK6P3tPDmfGdRmHee/"),
            method: Method::GET,
            headers: HeaderMap::new(),
            body: HttpBody::empty(),
            gas_limit: 550000,
        }),
    );

    let receipt = submit_operation(
        client,
        base_uri,
        call_op,
        "6c2858adb620f889949fa34bb2e13ba81f610e6707abb84b0242f6898470bccc",
    )
    .await;

    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::RunFunction(RunFunctionReceipt {
            body,
            status_code: StatusCode::OK,
            headers: _
        })) if &String::from_utf8(body.clone().unwrap()).unwrap() == "this is a big function"
    ));
}

// Check if the `DeployFunction`, `Deposit`, `FaDeposit` operations inside the inbox returned by the mock server
// is processed by the runtime.
async fn check_inbox_op(client: &Client, base_uri: &str) {
    let op = mock_deploy_op();
    let deploy_op_hash = op.hash().to_string();
    let receipt = poll_receipt(client, base_uri, &deploy_op_hash).await;
    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::DeployFunction(
            DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
        )) if addr.to_base58_check() == "KT1F2P4aqUVSrNEnk7F1RBd8fCeCpFQFubz7"
    ));

    let (deposit_op_hash, _) = mock_deposit_op();
    let receipt = poll_receipt(client, base_uri, deposit_op_hash).await;
    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::Deposit(DepositReceipt {
            account: Address::User(PublicKeyHash::Tz1(addr)),
            updated_balance,
        })) if addr.to_base58_check() == "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9" && updated_balance == 30000
    ));
    let balance =
        fetch_account_balance(client, base_uri, "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9")
            .await;
    assert_eq!(balance, 30000);

    let (deposit_fa_op_hash, _) = mock_deposit_fa_op();
    let receipt = poll_receipt(client, base_uri, deposit_fa_op_hash).await;
    assert!(matches!(
        receipt.result,
        ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
            receiver: Address::User(PublicKeyHash::Tz1(addr)),
            ticket_balance,
            ..
        })) if addr.to_base58_check() == "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN" && ticket_balance == 1000
    ));
}

async fn fetch_account_balance(client: &Client, base_uri: &str, address: &str) -> u64 {
    client
        .get(format!("{base_uri}/accounts/{address}/balance"))
        .send()
        .await
        .unwrap()
        .json::<u64>()
        .await
        .unwrap()
}

// Mocking the rollup node rpc

fn make_mock_rollup_rpc_server(url: String) -> JoinHandle<()> {
    let filter = make_mock_monitor_blocks_filter().or(make_mock_global_block_filter());
    let addr = url.parse::<std::net::SocketAddr>().unwrap();
    let server = warp::serve(filter).bind(addr);
    task::spawn(server)
}

pub(crate) fn make_mock_monitor_blocks_filter(
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("global" / "monitor_blocks").map(|| {
        let data_stream = stream::iter(vec![Ok::<Bytes, Infallible>(Bytes::from(
            "{\"level\": 123}\n",
        ))]);
        warp::reply::Response::new(Body::wrap_stream(data_stream))
    })
}

pub(crate) fn make_mock_global_block_filter(
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("global" / "block" / u32).map(|_| {
        let deploy_op = mock_deploy_op();
        let (_, deposit_op) = mock_deposit_op();
        let (_, deposit_fa_op) = mock_deposit_fa_op();
        let response = BlockResponse {
            messages: vec![
                &hex_start_of_level_message(),
                &hex_info_per_level_message(),
                &hex_external_message(deploy_op),
                deposit_op,
                deposit_fa_op,
                &hex_end_of_level_message(),
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        };
        warp::reply::json(&response)
    })
}

fn mock_deploy_op() -> SignedOperation {
    let KeyPair(alice_pk, alice_sk) = alice_keys();
    let code = r#"
        const handler = async () => {{
            return new Response();
        }};
        export default handler;
        "#;

    let deploy_fn = DeployFunction {
        function_code: ParsedCode::try_from(code.to_string()).unwrap(),
        account_credit: 0,
    };
    let op = Operation {
        public_key: alice_pk.clone(),
        nonce: 0.into(),
        content: deploy_fn.into(),
    };
    SignedOperation::new(alice_sk.sign(op.hash()).unwrap(), op.clone())
}

/// mock deposit op to transfer 30000 mutez to tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9
fn mock_deposit_op() -> (&'static str, &'static str) {
    let op = "0000050507070a000000160000c4ecf33f52c7b89168cfef8f350818fee1ad08e807070a000000160146d83d8ef8bce4d8c60a96170739c0269384075a00070707070000030600b0d40354267463f8cf2844e4d8b20a76f0471bcb2137fd0002298c03ed7d454a101eb7022bc95f7e5f41ac78c3ea4c18195bcfac262dcb29e3d803ae74681739";
    let op_hash = "09952d767f9ebf76b0edd3f837596b71f3b8193f13be4cbfb09e5eec691bbde3";
    (op_hash, op)
}

#[test]
fn mock_deposit_op_hash_matches_actual_hash() {
    let (op_hash, _) = mock_deposit_op();
    let inbox_id = InboxId {
        l1_level: 123,
        l1_message_id: 3,
    };
    assert_eq!(inbox_id.hash().to_string(), op_hash);
}

/// mock fa deposit op to transfer 1000 FA token to `tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN`
fn mock_deposit_fa_op() -> (&'static str, &'static str) {
    let op = "0000050807070a000000160000e7670f32038107a59a2b9cfefae36ea21f5aa63c070705090a0000001601238f371da359b757db57238e9f27f3c48234defa0007070a0000001601207905b1a5abdace0a6b5bff0d71a467d5b85cf500070707070001030600a80f9424c685d3f69801ff6e3f2cfb74b250f00988e100e7670f32038107a59a2b9cfefae36ea21f5aa63cc3ea4c18195bcfac262dcb29e3d803ae74681739";
    let op_hash = "89873bc722b5a0744657ed0bd6251f30989bea2059a52d080308c1d21923b053";
    (op_hash, op)
}

#[test]
fn mock_fa_deposit_op_hash_matches_actual_hash() {
    let (op_hash, _) = mock_deposit_fa_op();
    let inbox_id = InboxId {
        l1_level: 123,
        l1_message_id: 4,
    };
    assert_eq!(inbox_id.hash().to_string(), op_hash);
}

async fn check_worker_health(client: &Client, base_uri: &str) {
    let res = client
        .get(format!("{base_uri}/worker/health"))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}

async fn call_function_and_stream_logs(base_uri: &str) {
    let uri = base_uri.to_string();
    let h = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        let client = Client::new();
        call_function(&client, &uri).await;
    });

    let res = reqwest::get(format!(
        "{base_uri}/logs/KT1Lk9dy6cfWTQdB89rFK6P3tPDmfGdRmHee/stream"
    ))
    .await
    .unwrap();

    let mut found_message = false;
    let mut body = res.bytes_stream();
    for _ in 0..20 {
        if let Some(Ok(b)) = body.next().await {
            let s = String::from_utf8(b.to_vec()).unwrap().replace("data: ", "");
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(&s) {
                if m["text"].as_str().is_some_and(|v| v.contains("debug message here")) && m["requestId"] == serde_json::json!("6c2858adb620f889949fa34bb2e13ba81f610e6707abb84b0242f6898470bccc") {
                    found_message = true;
                    break;
                }
            }
        }
    }
    if let Err(e) = h.await {
        panic!("call_function panicked: {e}");
    }
    assert!(found_message, "did not find message in log stream");
}

// Utilities for encoding various inbox messages to hex strings.
#[cfg(test)]
pub mod inbox_utils {
    use super::*;
    use tezos_crypto_rs::hash::{BlockHash, HashTrait};
    use tezos_smart_rollup::{
        inbox::{InfoPerLevel, InternalInboxMessage},
        michelson::Michelson,
        types::Timestamp,
    };

    // Returns the hex-encoded serialized StartOfLevel inbox message.
    pub fn hex_start_of_level_message() -> String {
        let message =
            InboxMessage::<RollupType>::Internal(InternalInboxMessage::StartOfLevel);
        inbox_message_to_hex(message)
    }

    // Returns the hex-encoded serialized EndOfLevel inbox message.
    pub fn hex_end_of_level_message() -> String {
        let message =
            InboxMessage::<RollupType>::Internal(InternalInboxMessage::EndOfLevel);
        inbox_message_to_hex(message)
    }

    // Returns the hex-encoded serialized InfoPerLevel inbox message.
    pub fn hex_info_per_level_message() -> String {
        let message = InboxMessage::<RollupType>::Internal(
            InternalInboxMessage::InfoPerLevel(info_per_level().clone()),
        );
        inbox_message_to_hex(message)
    }

    // Returns the hex-encoded serialized external message for a given SignedOperation.
    pub fn hex_external_message(op: SignedOperation) -> String {
        let message = op.encode().unwrap();
        let external_message = ExternalMessageFrame::Targetted {
            address: SmartRollupAddress::from_b58check(JSTZ_ROLLUP_ADDRESS).unwrap(),
            contents: message,
        };
        let mut payload = Vec::new();
        external_message
            .bin_write(&mut payload)
            .expect("serialization of external payload failed");
        let external_message = InboxMessage::External::<RollupType>(&payload);
        inbox_message_to_hex(external_message)
    }

    fn info_per_level() -> InfoPerLevel {
        InfoPerLevel {
            predecessor: BlockHash::try_from_bytes(&[0; 32]).unwrap(),
            predecessor_timestamp: Timestamp::from(0),
        }
    }

    fn inbox_message_to_hex<T: Michelson>(message: InboxMessage<T>) -> String {
        let mut result = Vec::new();
        message
            .serialize(&mut result)
            .expect("serialization of message failed");
        hex::encode(result)
    }
}
