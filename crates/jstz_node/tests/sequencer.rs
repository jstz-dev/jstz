use axum::http::{HeaderMap, Method, StatusCode, Uri};
use bytes::Bytes;
use jstz_crypto::{
    public_key_hash::PublicKeyHash,
    smart_function_hash::{Kt1Hash, SmartFunctionHash},
};
use jstz_kernel::inbox::{
    parse_inbox_message_hex, Message, ParsedInboxMessage, RollupType,
};
use jstz_node::sequencer::{inbox::api::BlockResponse, runtime::JSTZ_ROLLUP_ADDRESS};
use jstz_proto::{
    context::account::{Address, Nonce},
    executor::fa_deposit::FaDepositReceipt,
    operation::{
        internal::InboxId, Content, DeployFunction, InternalOperation, Operation,
        RunFunction, SignedOperation,
    },
    receipt::{
        DeployFunctionReceipt, DepositReceipt, Receipt, ReceiptContent, ReceiptResult,
        RunFunctionReceipt,
    },
    HttpBody,
};
use jstz_utils::{inbox_builder::InboxBuilder, test_util::alice_keys, KeyPair};
use octez::unused_port;
use reqwest::Client;
use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    process::{Child, Command},
};
use tempfile::{NamedTempFile, TempDir};
use tokio_stream::StreamExt;

use futures_util::stream;
use inbox_utils::*;
use std::convert::Infallible;
use tezos_smart_rollup::inbox::InboxMessage;
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
async fn run_native_sequencer() {
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

    let deploy_op = mock_deploy_op();
    let (deposit_op_hash, deposit_op) = mock_deposit_op(
        "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9",
        30000,
        InboxId {
            l1_level: 123,
            // message_id is 3 because it is the 3rd message (0-based index) in the message
            // vector below.
            l1_message_id: 3,
        },
    );
    let (_, deposit_fa_op) = mock_deposit_fa_op();
    let inbox_messages = HashMap::from_iter([(
        123,
        vec![
            hex_start_of_level_message(),
            hex_info_per_level_message(),
            hex_external_message(deploy_op),
            deposit_op.to_string(),
            deposit_fa_op.to_string(),
            hex_end_of_level_message(),
        ],
    )]);
    let _rollup_rpc =
        make_mock_rollup_rpc_server(DEFAULT_ROLLUP_NODE_RPC.to_string(), inbox_messages);

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
                "--ticketer-address",
                "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ",
            ])
            .spawn()
            .unwrap(),
    );

    let client = Client::new();

    check_mode(&client, &base_uri).await;
    check_worker_health(&client, &base_uri).await;
    deploy_function(&client, &base_uri).await;
    call_function_and_stream_logs(&base_uri).await;

    // check if inbox messages are processed
    check_inbox_op(&client, &base_uri).await;
    check_deposit(
        &client,
        &base_uri,
        &deposit_op_hash,
        "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9",
        30000,
    )
    .await;

    check_worker_health(&client, &base_uri).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_native_sequencer() {
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
    let runtime_db_file = NamedTempFile::new().unwrap();
    let inbox_checkpoint_file = NamedTempFile::new().unwrap();
    let jstz_node_port = unused_port();
    let base_uri = format!("http://127.0.0.1:{jstz_node_port}");
    let rollup_rpc_port = unused_port();

    let deposit_recipient = "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9";
    let (deposit_op_hash, deposit_op) = mock_deposit_op(
        deposit_recipient,
        1000,
        InboxId {
            l1_level: 123,
            // message_id is 1 because it is the 1st message (0-based index) in the message
            // vector below.
            l1_message_id: 1,
        },
    );
    let mut inbox_messages = HashMap::from_iter([(
        123,
        vec![
            hex_start_of_level_message(),
            deposit_op,
            hex_end_of_level_message(),
        ],
    )]);
    let rollup_rpc = make_mock_rollup_rpc_server(
        format!("127.0.0.1:{rollup_rpc_port}"),
        inbox_messages.clone(),
    );
    let bin_path = assert_cmd::cargo::cargo_bin("jstz-node");

    let launch_jstz_node = || {
        ChildWrapper(
            Command::new(&bin_path)
                .args([
                    "run",
                    "--port",
                    &jstz_node_port.to_string(),
                    "--rollup-endpoint",
                    &format!("http://127.0.0.1:{rollup_rpc_port}"),
                    "--preimages-dir",
                    tmp_dir.path().to_str().unwrap(),
                    "--kernel-log-path",
                    log_file.path().to_str().unwrap(),
                    "--mode",
                    "sequencer",
                    "--injector-key-file",
                    injector_file.path().to_str().unwrap(),
                    "--runtime-db-path",
                    runtime_db_file.path().to_str().unwrap(),
                    "--inbox-checkpoint-path",
                    inbox_checkpoint_file.path().to_str().unwrap(),
                    "--ticketer-address",
                    "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ",
                ])
                .spawn()
                .unwrap(),
        )
    };

    let c = launch_jstz_node();
    let client = Client::new();

    let smart_function_exists = async || -> bool {
        let res = fetch_account_balance(
            &client,
            &base_uri,
            "KT1Lk9dy6cfWTQdB89rFK6P3tPDmfGdRmHee",
        )
        .await;
        match res {
            Ok(v) => {
                assert_eq!(v, 0);
                true
            }
            Err(e) => {
                assert_eq!(e.status().unwrap(), 404);
                false
            }
        }
    };

    check_worker_health(&client, &base_uri).await;
    assert!(!smart_function_exists().await);
    deploy_function(&client, &base_uri).await;
    assert!(smart_function_exists().await);
    check_deposit(
        &client,
        &base_uri,
        &deposit_op_hash,
        deposit_recipient,
        1000,
    )
    .await;

    // Kill jstz node and rpc server
    drop(c);
    rollup_rpc.abort();
    // Wait until jstz node is shut down
    jstz_utils::poll(60, 500, || async {
        client.get(format!("{base_uri}/health")).send().await.err()
    })
    .await
    .expect("should get connection error");
    // wait until rpc server is shut down
    jstz_utils::poll(60, 500, || async {
        client
            .get(format!(
                "http://127.0.0.1:{rollup_rpc_port}/global/monitor_blocks"
            ))
            .send()
            .await
            .err()
    })
    .await
    .expect("should get connection error");

    // Restart mock rollup server with one new block
    let (deposit_op_hash, deposit_op) = mock_deposit_op(
        deposit_recipient,
        1,
        InboxId {
            l1_level: 124,
            l1_message_id: 1,
        },
    );
    inbox_messages.insert(
        124,
        vec![
            hex_start_of_level_message(),
            deposit_op,
            hex_end_of_level_message(),
        ],
    );
    let _rollup_rpc = make_mock_rollup_rpc_server(
        format!("127.0.0.1:{rollup_rpc_port}"),
        inbox_messages,
    );
    // Restart jstz node
    let _c = launch_jstz_node();
    check_worker_health(&client, &base_uri).await;
    // Smart function should still exist because the sequencer should read runtime db
    // from the designated db file
    assert!(smart_function_exists().await);
    // Account balance should be 1000 (deposit in level 123) + 1 (deposit in level 124).
    // Deposit in level 123 should not be replayed even if the monitor_blocks endpoint
    // returns level 123 first
    check_deposit(
        &client,
        &base_uri,
        &deposit_op_hash,
        deposit_recipient,
        1001,
    )
    .await;
}

#[cfg_attr(
    not(feature = "riscv_test"),
    ignore = "RISCV tests are not enabled by default due to memory usage"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_riscv_sequencer() {
    let riscv_kernel_path =
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/riscv_kernel");
    let tmp_dir = TempDir::new().unwrap();
    let log_file = NamedTempFile::new().unwrap();
    let mut injector_file = NamedTempFile::new().unwrap();
    injector_file
        // using the kernel's default injector public key in build.rs
        .write_all(
            br#"{
            "public_key": "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            "secret_key": "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh"
}"#,
        )
        .unwrap();
    injector_file.flush().unwrap();
    let port = unused_port();
    let rollup_rpc_port = unused_port();
    let base_uri = format!("http://127.0.0.1:{port}");
    let deploy_op = mock_deploy_op();
    let (deposit_op_hash, deposit_op) = mock_deposit_op(
        "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9",
        30000,
        InboxId {
            l1_level: 123,
            // message_id is 3 because it is the 3rd message (0-based index) in the message
            // vector below.
            l1_message_id: 3,
        },
    );
    let (_, deposit_fa_op) = mock_deposit_fa_op();
    let inbox_messages = HashMap::from_iter([(
        123,
        vec![
            hex_start_of_level_message(),
            hex_info_per_level_message(),
            hex_external_message(deploy_op),
            deposit_op.to_string(),
            deposit_fa_op.to_string(),
            hex_end_of_level_message(),
        ],
    )]);
    let _rollup_rpc = make_mock_rollup_rpc_server(
        format!("127.0.0.1:{rollup_rpc_port}"),
        inbox_messages.clone(),
    );

    let bin_path = assert_cmd::cargo::cargo_bin("jstz-node");
    let _c = ChildWrapper(
        Command::new(bin_path)
            .args([
                "run",
                "--port",
                &port.to_string(),
                "--rollup-node-rpc-addr",
                "127.0.0.1",
                "--rollup-node-rpc-port",
                &rollup_rpc_port.to_string(),
                "--preimages-dir",
                tmp_dir.path().to_str().unwrap(),
                "--debug-log-path",
                log_file.path().to_str().unwrap(),
                "--mode",
                "sequencer",
                "--injector-key-file",
                injector_file.path().to_str().unwrap(),
                "--riscv-kernel-path",
                riscv_kernel_path.to_str().unwrap(),
                "--rollup-address",
                "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
                "--ticketer-address",
                "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ",
            ])
            .spawn()
            .unwrap(),
    );

    let client = Client::new();

    check_mode(&client, &base_uri).await;
    check_worker_health(&client, &base_uri).await;
    deploy_function(&client, &base_uri).await;
    call_function_and_stream_logs(&base_uri).await;

    // check if inbox messages are processed
    check_inbox_op(&client, &base_uri).await;
    check_deposit(
        &client,
        &base_uri,
        &deposit_op_hash,
        "tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9",
        30000,
    )
    .await;

    check_worker_health(&client, &base_uri).await;
}

async fn check_mode(client: &Client, base_uri: &str) {
    let res = jstz_utils::poll(60, 500, || async {
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
    jstz_utils::poll(60, 500, || async {
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
    let deploy_op = raw_operation(0, Content::DeployFunction(DeployFunction {function_code: format!("const handler = async () => {{ const s = \"{}\"; console.log(\"debug message here\"); return new Response(\"this is a big function\"); }}; export default handler;\n", "a".repeat(8000)), account_credit: 0}));

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

// Check if a `Deposit` operation from the inbox is executed correctly.
async fn check_deposit(
    client: &Client,
    base_uri: &str,
    op_hash: &str,
    recipient: &str,
    expected_balance_mutez: u64,
) {
    let receipt = poll_receipt(client, base_uri, op_hash).await;
    assert!(
        matches!(
            &receipt.result,
            ReceiptResult::Success(ReceiptContent::Deposit(DepositReceipt {
                account: Address::User(PublicKeyHash::Tz1(addr)),
                updated_balance,
            })) if addr.to_base58_check() == recipient && updated_balance == &expected_balance_mutez
        ),
        "unexpected result: {:?}",
        receipt.result
    );

    let balance = fetch_account_balance(client, base_uri, recipient)
        .await
        .unwrap();
    assert_eq!(balance, expected_balance_mutez);
}

// Check if the `DeployFunction` and `FaDeposit` operations inside the inbox returned by the mock server
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

async fn fetch_account_balance(
    client: &Client,
    base_uri: &str,
    address: &str,
) -> reqwest::Result<u64> {
    client
        .get(format!("{base_uri}/accounts/{address}/balance"))
        .send()
        .await?
        .error_for_status()?
        .json::<u64>()
        .await
}

// Mocking the rollup node rpc

fn make_mock_rollup_rpc_server(
    url: String,
    messages: HashMap<u32, Vec<String>>,
) -> JoinHandle<()> {
    let levels = messages.keys().copied().collect();
    let filter = make_mock_monitor_blocks_filter(levels)
        .or(make_mock_global_block_filter(messages));
    let addr = url.parse::<std::net::SocketAddr>().unwrap();
    let server = warp::serve(filter).bind(addr);
    task::spawn(server)
}

pub(crate) fn make_mock_monitor_blocks_filter(
    levels: Vec<u32>,
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("global" / "monitor_blocks").map(move || {
        let mut content = String::new();
        for v in levels.clone() {
            content.push_str(&format!("{{\"level\": {v}}}\n"));
        }
        let data_stream =
            stream::iter(vec![Ok::<Bytes, Infallible>(Bytes::from(content))]);
        warp::reply::Response::new(Body::wrap_stream(data_stream))
    })
}

pub(crate) fn make_mock_global_block_filter(
    message_map: HashMap<u32, Vec<String>>,
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let message_map = std::sync::Arc::new(message_map);
    warp::path!("global" / "block" / u32).map(move |level: u32| {
        let messages = match message_map.clone().get(&level) {
            Some(messages) => messages.to_owned(),
            // Default to nothing when messages for the level are not provided
            None => vec![hex_start_of_level_message(), hex_end_of_level_message()],
        };
        warp::reply::json(&BlockResponse { messages })
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
        function_code: code.to_string(),
        account_credit: 0,
    };
    let op = Operation {
        public_key: alice_pk.clone(),
        nonce: 0.into(),
        content: deploy_fn.into(),
    };
    SignedOperation::new(alice_sk.sign(op.hash()).unwrap(), op.clone())
}

// Note that `inbox_id` must match the level and the index of the actual level and index of
// the message when it gets inserted into the mock rollup rpc server.
fn mock_deposit_op(dst: &str, amount_mutez: u64, inbox_id: InboxId) -> (String, String) {
    struct DummyLogger;
    impl jstz_core::host::WriteDebug for DummyLogger {
        fn write_debug(&self, _msg: &str) {}
    }

    // Default rollup address (will be made configurable later)
    let rollup_addr =
        SmartRollupAddress::from_b58check("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK")
            .unwrap();
    // Default ticketer address (will be made configurable later)
    let ticketer = tezos_crypto_rs::hash::ContractKt1Hash::from_base58_check(
        "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ",
    )
    .unwrap();

    let mut builder = InboxBuilder::new(
        rollup_addr.clone(),
        Some(ticketer.clone()),
        #[cfg(feature = "v2_runtime")]
        None,
    );
    builder
        .deposit_from_l1(
            &jstz_utils::inbox_builder::Account {
                nonce: Nonce(0),
                // sk and pk do not matter here as they are not referenced by this deposit method
                sk: jstz_mock::sk1(),
                pk: jstz_mock::pk1(),
                address: Address::from_base58(dst).unwrap(),
            },
            amount_mutez,
        )
        .unwrap();
    let inbox = builder.build();
    let serde_json::Value::String(inbox_msg) =
        serde_json::to_value(inbox.0.first().unwrap().first().unwrap()).unwrap()
    else {
        // Unreachable because the inbox message is supposed to be serialised to a hex string.
        unreachable!()
    };

    // Parse the raw message to derive operation hash
    let parsed_op = parse_inbox_message_hex(
        &DummyLogger,
        inbox_id,
        &inbox_msg,
        &ticketer,
        rollup_addr.hash(),
    )
    .unwrap()
    .content;
    let ParsedInboxMessage::JstzMessage(Message::Internal(InternalOperation::Deposit(d))) =
        parsed_op
    else {
        // Unreachable because the inbox message built above is supposed to be a deposit message
        unreachable!()
    };

    (d.hash().to_string(), inbox_msg)
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
    let res = jstz_utils::poll(60, 500, || async {
        client
            .get(format!("{base_uri}/worker/health"))
            .send()
            .await
            .ok()
    })
    .await
    .expect("should get response");

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
    use jstz_kernel::inbox::encode_signed_operation;
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
        let bytes = encode_signed_operation(
            &op,
            &SmartRollupAddress::from_b58check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        )
        .unwrap();
        hex::encode(bytes)
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
