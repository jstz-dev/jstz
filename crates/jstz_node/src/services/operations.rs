use std::fs;
use std::path;
use std::sync::Arc;
use std::sync::RwLock;

use crate::sequencer::queue::OperationQueue;
use crate::services::accounts::get_account_nonce;
use crate::RunMode;
use jstz_kernel::inbox::Message;
use jstz_kernel::inbox::ParsedInboxMessage;

use super::error::{ServiceError, ServiceResult};
use super::utils::StoreWrapper;
use super::{AppState, Service};
use anyhow::anyhow;
use anyhow::Context;
use axum::{
    extract::{Path, State},
    Json,
};

use jstz_core::reveal_data::{PreimageHash, RevealData, MAX_REVEAL_SIZE};
use jstz_core::BinEncodable;
use jstz_proto::operation::{Content, Operation, SignedOperation};
use jstz_proto::receipt::Receipt;
use jstz_utils::KeyPair;
use octez::OctezRollupClient;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;

use tokio::task::JoinSet;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// The maximum operation size in bytes that can be directly included without using the reveal mechanism.
const MAX_DIRECT_OPERATION_SIZE: usize = 3915;

pub struct OperationsService;

const OPERATIONS_TAG: &str = "Operations";

type HexEncodedOperationHash = String;

// Given a large operation, encode it into preimages and store them in the rollup's preimages directory
async fn prepare_rlp_operation(
    operation: &SignedOperation,
    signer: KeyPair,
    store: StoreWrapper,
    rollup_preimages_dir: &path::Path,
) -> ServiceResult<SignedOperation> {
    let reveal_type = operation
        .verify_ref()
        .map_err(|e| anyhow!("Invalid operation: {}", e))?
        .content()
        .try_into()
        .map_err(|e| {
            ServiceError::BadRequest(format!(
                "Large payload operation not supported: {e}",
            ))
        })?;

    let mut write_tasks = JoinSet::new();
    let save_preimages = |hash: PreimageHash, preimage: Vec<u8>| {
        let path = rollup_preimages_dir.join(hash.to_string());
        write_tasks.spawn(async move { fs::write(&path, preimage) });
    };
    let KeyPair(public_key, secret_key) = signer;
    let root_hash = RevealData::encode_and_prepare_preimages(operation, save_preimages)
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("failed to prepare reveal large payload operation")?;
    write_tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, _>>()
        .map_err(|e| anyhow!("failed to save preimages: {e}"))?;

    let nonce = get_account_nonce(store, &public_key.hash())
        .await?
        .unwrap_or_default();
    let rlp_operation = Operation {
        public_key,
        nonce,
        content: Content::new_reveal_large_payload(
            root_hash,
            reveal_type,
            operation.hash(),
        ),
    };
    let signature = secret_key
        .sign(rlp_operation.hash())
        .map_err(|e| anyhow!("failed to sign reval large payload operation: {e}"))?;
    Ok(SignedOperation::new(signature, rlp_operation))
}

// Encode an operation. if the operation is too large, encode it into a reveal large payload operation
async fn encode_operation(
    operation: SignedOperation,
    injector: KeyPair,
    store: StoreWrapper,
    rollup_preimages_dir: &path::Path,
) -> ServiceResult<(SignedOperation, Vec<u8>)> {
    let encoded_op = operation
        .encode()
        .map_err(|e| anyhow!("Failed to serialize operation: {e}"))?;

    let (op, contents) = match encoded_op.len() {
        size if size <= MAX_DIRECT_OPERATION_SIZE => (operation, encoded_op),
        size if size <= MAX_REVEAL_SIZE => {
            let op =
                prepare_rlp_operation(&operation, injector, store, rollup_preimages_dir)
                    .await?;
            let encoded_op = op
                .encode()
                .map_err(|e| anyhow!("Failed to encode rlp operation: {e}"))?;
            (op, encoded_op)
        }
        size => Err(anyhow!(
            "Operation size exceeds maximum allowed size ({} bytes > {} MB)",
            size,
            MAX_REVEAL_SIZE / 1024 / 1024
        ))?,
    };

    Ok((op, contents))
}

/// Inject an operation into Jstz
#[utoipa::path(
        post,
        path = "",
        tag = OPERATIONS_TAG,
        responses(
            (status = 200, description = "Operation successfully injected"),
            (status = 400),
            (status = 500)
        )
    )]
async fn inject(
    State(AppState {
        rollup_client,
        rollup_preimages_dir,
        injector,
        mode,
        queue,
        runtime_db,
        ..
    }): State<AppState>,
    Json(operation): Json<SignedOperation>,
) -> ServiceResult<()> {
    let store = StoreWrapper::new(mode.clone(), rollup_client.clone(), runtime_db);
    let (operation, encoded_operation) =
        encode_operation(operation, injector, store, &rollup_preimages_dir).await?;
    match mode {
        RunMode::Default => {
            inject_rollup_message(encoded_operation, &rollup_client).await?;
        }
        RunMode::Sequencer { .. } => {
            insert_operation_queue(&queue, operation).await?;
        }
    }
    Ok(())
}

async fn inject_rollup_message(
    contents: Vec<u8>,
    rollup_client: &OctezRollupClient,
) -> ServiceResult<()> {
    let address = rollup_client.get_rollup_address().await?;
    let message_frame = ExternalMessageFrame::Targetted { address, contents };
    let mut binary_contents = Vec::new();
    message_frame
        .bin_write(&mut binary_contents)
        .map_err(|_| anyhow!("Failed to write binary frame"))?;
    rollup_client.batcher_injection([binary_contents]).await?;
    Ok(())
}

async fn insert_operation_queue(
    queue: &Arc<RwLock<OperationQueue>>,
    operation: SignedOperation,
) -> ServiceResult<()> {
    queue
        .write()
        .map_err(|e| {
            ServiceError::FromAnyhow(anyhow::anyhow!(
                "failed to insert operation to the queue: {e}"
            ))
        })?
        .insert(ParsedInboxMessage::JstzMessage(Message::External(
            operation,
        )))
        .map_err(|e| ServiceError::ServiceUnavailable(Some(e)))?;
    Ok(())
}

/// Get the receipt of an operation
#[utoipa::path(
        get,
        path = "/{operation_hash}/receipt",
        tag = OPERATIONS_TAG,
        params(
            ("operation_hash" = String, description = "Operation hash")
        ),
        responses(
            (status = 200, body = Receipt),
            (status = 400),
            (status = 500)
        )
    )]
async fn receipt(
    State(AppState {
        rollup_client,
        mode,
        runtime_db,
        ..
    }): State<AppState>,
    Path(hash): Path<String>,
) -> ServiceResult<Json<Receipt>> {
    let key = format!("/jstz_receipt/{hash}");

    let store = StoreWrapper::new(mode, rollup_client, runtime_db);
    let value = store.get_value(key).await?;

    let receipt = match value {
        Some(value) => Receipt::decode(value.as_slice())
            .map_err(|_| anyhow!("Failed to deserialize receipt"))?,
        None => Err(ServiceError::NotFound)?,
    };

    Ok(Json(receipt))
}

/// Returns the hex encoded hash of an Operation
#[utoipa::path(
        post,
        path = "/hash",
        tag = OPERATIONS_TAG,
        responses(
            (status = 200, body = HexEncodedOperationHash),
            (status = 400),
            (status = 500)
        )
    )]
async fn hash_operation(
    Json(operation): Json<Operation>,
) -> ServiceResult<Json<HexEncodedOperationHash>> {
    Ok(Json(format!("{}", operation.hash())))
}

impl Service for OperationsService {
    fn router_with_openapi() -> OpenApiRouter<AppState> {
        let routes = OpenApiRouter::new()
            .routes(routes!(inject))
            .routes(routes!(receipt))
            .routes(routes!(hash_operation));

        OpenApiRouter::new().nest("/operations", routes)
    }
}

#[cfg(test)]
mod tests {

    use std::borrow::BorrowMut;
    use std::path::PathBuf;
    use std::{fs, path::Path};

    use axum::{
        body::Body,
        http::{HeaderMap, Method, Request, Uri},
    };
    use jstz_core::reveal_data::MAX_REVEAL_SIZE;
    use jstz_core::BinEncodable;
    use jstz_crypto::{
        hash::Hash,
        public_key::PublicKey,
        public_key_hash::PublicKeyHash,
        secret_key::SecretKey,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use jstz_proto::operation::{RevealLargePayload, RevealType};
    use jstz_proto::receipt::{ReceiptContent, ReceiptResult};
    use jstz_proto::{
        context::account::{Amount, Nonce},
        operation::{Content, DeployFunction, Operation, RunFunction, SignedOperation},
        receipt::{DeployFunctionReceipt, Receipt},
        runtime::ParsedCode,
    };
    use jstz_utils::KeyPair;
    use octez::OctezRollupClient;
    use tempfile::{NamedTempFile, TempDir};
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tower::ServiceExt;

    use crate::config::RuntimeEnv;
    use crate::services::utils::StoreWrapper;
    use crate::{
        services::{
            error::ServiceError,
            operations::{encode_operation, OperationsService},
            Service,
        },
        utils::tests::{dummy_receipt, mock_app_state},
        RunMode,
    };
    use jstz_kernel::inbox::Message;
    use jstz_kernel::inbox::ParsedInboxMessage;

    use super::MAX_DIRECT_OPERATION_SIZE;

    fn bootstrap1() -> (PublicKeyHash, PublicKey, SecretKey) {
        (
            PublicKeyHash::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx").unwrap(),
            PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            )
            .unwrap(),
        )
    }

    fn make_signed_op(content: Content) -> SignedOperation {
        let (_, pk, sk) = bootstrap1();
        let deploy_op = Operation {
            public_key: pk,
            nonce: Nonce(0),
            content,
        };
        let sig = sk.sign(deploy_op.hash()).unwrap();
        SignedOperation::new(sig, deploy_op)
    }

    fn mock_code(size: usize) -> ParsedCode {
        // SAFETY: This code is never interpreted (so does not need to be parsable)
        unsafe { ParsedCode::new_unchecked("a".repeat(size)) }
    }

    fn get_dir_size(path: &Path) -> u64 {
        let mut size = 0;
        for entry_result in fs::read_dir(path).unwrap() {
            let entry = entry_result.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                // Recurse into subdirectories
                size += get_dir_size(&entry.path());
            } else {
                // Add up file sizes
                size += metadata.len();
            }
        }
        size
    }

    fn inject_operation_request(op: SignedOperation) -> Request<Body> {
        Request::builder()
            .uri("/operations")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&op).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn encodes_normal_operation() {
        let (_, pk, sk) = bootstrap1();
        let client = OctezRollupClient::new("http://localhost:8732".to_string());
        let code = mock_code(1);
        let operation = make_signed_op(Content::DeployFunction(DeployFunction {
            account_credit: Amount::default(),
            function_code: code,
        }));
        let key_pair = KeyPair(pk, sk);
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StoreWrapper::Rollup(client);
        let result = encode_operation(operation, key_pair, store, temp_dir.path()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn encodes_large_payload_operation_and_make_data_available() {
        let (pkh, pk, sk) = bootstrap1();
        let mut server = mockito::Server::new_async().await;
        let url = format!(
            "/global/block/head/durable/wasm_2_0_0/value?key=/jstz_account/{pkh}"
        );
        server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_body(r#""01000000000000000000000000000000000000000901000000000000636f6e7374204b4559203d2022636f756e746572223b0a0a636f6e73742068616e646c6572203d202829203d3e207b0a20206c657420636f756e746572203d204b762e676574284b4559293b0a2020636f6e736f6c652e6c6f672860436f756e7465723a20247b636f756e7465727d60293b0a202069662028636f756e746572203d3d3d206e756c6c29207b0a20202020636f756e746572203d20303b0a20207d20656c7365207b0a20202020636f756e7465722b2b3b0a20207d0a20204b762e736574284b45592c20636f756e746572293b0a202072657475726e206e657720526573706f6e736528293b0a7d3b0a0a6578706f72742064656661756c742068616e646c65723b0a""#)
            .create();
        let client = OctezRollupClient::new(server.url());

        let temp_dir = tempfile::tempdir().unwrap();
        let code = mock_code(MAX_DIRECT_OPERATION_SIZE);
        let code_size: u64 = code.len() as u64;
        let operation = make_signed_op(Content::DeployFunction(DeployFunction {
            account_credit: Amount::default(),
            function_code: code,
        }));
        let key_pair = KeyPair(pk, sk);
        let store = StoreWrapper::Rollup(client);
        let result = encode_operation(operation, key_pair, store, temp_dir.path()).await;
        assert!(result.is_ok());
        let dir_size = get_dir_size(temp_dir.path());
        assert!(
            dir_size > code_size,
            "Expected temp_dir to have some file data, but got size = 0"
        );
    }

    #[tokio::test]
    async fn encodes_operation_throws_if_operation_is_too_large() {
        let (_, pk, sk) = bootstrap1();
        let client = OctezRollupClient::new("http://localhost:8732".to_string());
        let code = mock_code(MAX_REVEAL_SIZE + 1);
        let operation = make_signed_op(Content::DeployFunction(DeployFunction {
            account_credit: Amount::default(),
            function_code: code,
        }));
        let key_pair = KeyPair(pk, sk);
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StoreWrapper::Rollup(client);
        let result = encode_operation(operation, key_pair, store, temp_dir.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn encodes_large_payload_operation_throws_if_write_preimages_fails() {
        let (pkh, pk, sk) = bootstrap1();
        let mut server = mockito::Server::new_async().await;
        let url = format!(
            "/global/block/head/durable/wasm_2_0_0/value?key=/jstz_account/{pkh}"
        );
        server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_body(r#""01000000000000000000000000000000000000000901000000000000636f6e7374204b4559203d2022636f756e746572223b0a0a636f6e73742068616e646c6572203d202829203d3e207b0a20206c657420636f756e746572203d204b762e676574284b4559293b0a2020636f6e736f6c652e6c6f672860436f756e7465723a20247b636f756e7465727d60293b0a202069662028636f756e746572203d3d3d206e756c6c29207b0a20202020636f756e746572203d20303b0a20207d20656c7365207b0a20202020636f756e7465722b2b3b0a20207d0a20204b762e736574284b45592c20636f756e746572293b0a202072657475726e206e657720526573706f6e736528293b0a7d3b0a0a6578706f72742064656661756c742068616e646c65723b0a""#)
            .create();
        let client = OctezRollupClient::new(server.url());

        let code = mock_code(MAX_DIRECT_OPERATION_SIZE);
        let operation = make_signed_op(Content::DeployFunction(DeployFunction {
            account_credit: Amount::default(),
            function_code: code,
        }));
        let key_pair = KeyPair(pk, sk);
        let store = StoreWrapper::Rollup(client);
        let result =
            encode_operation(operation, key_pair, store, Path::new("invalid path")).await;
        assert!(result.is_err_and(|e| {
            matches!(
                e,
                ServiceError::FromAnyhow(e) if e.to_string().contains("failed to save preimages")
            )
        }));
    }

    #[tokio::test]
    async fn inject_default() {
        let mut server = mockito::Server::new_async().await;
        let mock_injection = server.mock("POST", "/local/batcher/injection").create();
        let mock_rollup_addr = server
            .mock("GET", "/global/smart_rollup_address")
            .with_body("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK")
            .create();

        let db_file = NamedTempFile::new().unwrap();
        let state = mock_app_state(
            &server.url(),
            PathBuf::default(),
            db_file.path().to_str().unwrap(),
            RunMode::Default,
        )
        .await;
        let queue = state.queue.clone();
        assert_eq!(queue.read().unwrap().len(), 0);
        let (router, _) = OperationsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let res = router
            .oneshot(inject_operation_request(make_signed_op(
                Content::RunFunction(RunFunction {
                    uri: Uri::from_static("http://http://"),
                    method: Method::HEAD,
                    headers: HeaderMap::new(),
                    body: None,
                    gas_limit: 0,
                }),
            )))
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        assert_eq!(queue.read().unwrap().len(), 0);
        mock_injection.assert();
        mock_rollup_addr.assert();
    }

    #[tokio::test]
    async fn inject_sequencer() {
        let db_file = NamedTempFile::new().unwrap();
        let state = mock_app_state(
            "",
            PathBuf::default(),
            db_file.path().to_str().unwrap(),
            RunMode::Sequencer {
                capacity: 0,
                debug_log_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                runtime_env: RuntimeEnv::Native,
            },
        )
        .await;
        let queue = state.queue.clone();
        assert_eq!(queue.read().unwrap().len(), 0);
        let (mut router, _) = OperationsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let dummy_op = make_signed_op(Content::RunFunction(RunFunction {
            uri: Uri::from_static("http://http://"),
            method: Method::HEAD,
            headers: HeaderMap::new(),
            body: None,
            gas_limit: 0,
        }));
        let res = router
            .borrow_mut()
            .oneshot(inject_operation_request(dummy_op.clone()))
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        assert_eq!(queue.read().unwrap().len(), 1);

        // sending the operation again should fail because the queue is full
        let res = router
            .borrow_mut()
            .oneshot(inject_operation_request(dummy_op))
            .await
            .unwrap();
        assert_eq!(res.status(), 503);
    }

    #[tokio::test]
    async fn inject_large_operation_sequencer() {
        let db_file = NamedTempFile::new().unwrap();
        let preimage_dir = TempDir::new().unwrap();
        let state = mock_app_state(
            "",
            preimage_dir.path().to_path_buf(),
            db_file.path().to_str().unwrap(),
            RunMode::Sequencer {
                capacity: 0,
                debug_log_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                runtime_env: RuntimeEnv::Native,
            },
        )
        .await;
        let queue = state.queue.clone();
        assert_eq!(queue.read().unwrap().len(), 0);
        let (mut router, _) = OperationsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let dummy_op = make_signed_op(Content::DeployFunction(DeployFunction {
            function_code: ParsedCode("a".repeat(4000)),
            account_credit: 0,
        }));
        let res = router
            .borrow_mut()
            .oneshot(inject_operation_request(dummy_op.clone()))
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        assert_eq!(queue.read().unwrap().len(), 1);
        let injected_op = match queue.write().unwrap().pop().unwrap() {
            ParsedInboxMessage::JstzMessage(Message::External(op)) => op,
            _ => panic!("invalid message type"),
        };
        let inner = injected_op.verify_ref().unwrap();
        matches!(
            &inner.content,
            Content::RevealLargePayload(RevealLargePayload {
                root_hash: _,
                reveal_type: RevealType::DeployFunction,
                original_op_hash
            }) if original_op_hash == &dummy_op.hash()
        );
    }

    #[tokio::test]
    async fn get_receipt_sequencer() {
        let smart_function_hash =
            ContractKt1Hash::from_base58_check("KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX")
                .unwrap();
        let receipt = dummy_receipt(smart_function_hash.clone());
        let op_hash = "9b15976cc8162fe39458739de340a1a95c59a9bcff73bd3c83402fad6352396e";
        let db_file = NamedTempFile::new().unwrap();
        let state = mock_app_state(
            "",
            PathBuf::default(),
            db_file.path().to_str().unwrap(),
            RunMode::Sequencer {
                capacity: 0,
                debug_log_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                runtime_env: RuntimeEnv::Native,
            },
        )
        .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_receipt/{op_hash}"),
                &hex::encode(receipt.encode().unwrap()),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                "/jstz_receipt/bad_value",
                &hex::encode(mock_code(10).encode().unwrap()),
            )
            .unwrap();

        let (mut router, _) = OperationsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();

        // good receipt
        let res = router
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .uri(format!("/operations/{op_hash}/receipt"))
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let receipt = serde_json::from_slice::<Receipt>(&bytes).unwrap();
        assert!(matches!(
            receipt.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr == smart_function_hash
        ));

        // bad receipt
        let res = router
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .uri("/operations/bad_value/receipt")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let error_message = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        assert_eq!(
            error_message,
            serde_json::json!({"error": "Failed to deserialize receipt"})
        );

        // non-existent receipt
        let res = router
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .uri("/operations/bad_hash/receipt")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }
}
