use std::fs;
use std::path;

use crate::config::KeyPair;
use crate::services::accounts::get_account_nonce;

use super::error::{ServiceError, ServiceResult};
use super::{AppState, Service};
use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    Json,
};

use jstz_core::reveal_data::{PreimageHash, RevealData, MAX_REVEAL_SIZE};
use jstz_core::BinEncodable;
use jstz_proto::operation::{Content, Operation, SignedOperation};
use jstz_proto::receipt::Receipt;
use octez::OctezRollupClient;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// The maximum operation size in bytes that can be directly included without using the reveal mechanism.
const MAX_DIRECT_OPERATION_SIZE: usize = 3915;

pub struct OperationsService;

const OPERATIONS_TAG: &str = "Operations";

type HexEncodedOperationHash = String;

// Given a large operation, encode it into preimages and store them in the rollup's preimages directory
async fn prepare_rlp_operation(
    rollup_client: &OctezRollupClient,
    signer: KeyPair,
    operation: &SignedOperation,
    rollup_preimages_dir: &path::Path,
) -> ServiceResult<SignedOperation> {
    let reveal_type = operation
        .verify_ref()
        .map_err(|_| anyhow!("Invalid operation"))?
        .content()
        .into();
    let save_preimages = |hash: PreimageHash, preimage: Vec<u8>| {
        let path = rollup_preimages_dir.join(hash.to_string());
        fs::write(&path, preimage)
            .unwrap_or_else(|_| panic!("failed to save preimage at: {:?}", path));
    };
    let KeyPair(public_key, secret_key) = signer;
    let root_hash =
        RevealData::encode_and_prepare_preimages(operation, save_preimages)
            .map_err(|_| anyhow!("failed to prepare reval large payload operation"))?;
    let nonce = get_account_nonce(rollup_client, &public_key.hash()).await?;
    let rlp_operation = Operation {
        public_key,
        nonce,
        content: Content::new_reval_large_payload(root_hash, reveal_type),
    };
    let signature = secret_key
        .sign(rlp_operation.hash())
        .map_err(|_| anyhow!("failed to sign reval large payload operation"))?;
    Ok(SignedOperation::new(signature, rlp_operation))
}

// Encode an operation. if the operation is too large, encode it into a reveal large payload operation
async fn encode_operation(
    operation: &SignedOperation,
    rollup_client: &OctezRollupClient,
    injector: KeyPair,
    rollup_preimages_dir: &path::Path,
) -> ServiceResult<Vec<u8>> {
    let encoded_op = operation
        .encode()
        .map_err(|_| anyhow!("Failed to serialize operation"))?;

    let contents = match encoded_op.len() {
        size if size <= MAX_DIRECT_OPERATION_SIZE => encoded_op,
        size if size <= MAX_REVEAL_SIZE => prepare_rlp_operation(
            rollup_client,
            injector,
            operation,
            rollup_preimages_dir,
        )
        .await?
        .encode()
        .map_err(|_| anyhow!("Failed to encode rlp operation"))?,
        size => Err(anyhow!(
            "Operation size exceeds maximum allowed size ({} bytes > {} MB)",
            size,
            MAX_REVEAL_SIZE / 1024 / 1024
        ))?,
    };

    Ok(contents)
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
        ..
    }): State<AppState>,
    Json(operation): Json<SignedOperation>,
) -> ServiceResult<()> {
    let contents =
        encode_operation(&operation, &rollup_client, injector, &rollup_preimages_dir)
            .await?;

    let address = rollup_client.get_rollup_address().await?;
    let message_frame = ExternalMessageFrame::Targetted { address, contents };
    let mut binary_contents = Vec::new();
    message_frame
        .bin_write(&mut binary_contents)
        .map_err(|_| anyhow!("Failed to write binary frame"))?;
    rollup_client.batcher_injection([binary_contents]).await?;
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
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(hash): Path<String>,
) -> ServiceResult<Json<Receipt>> {
    let key = format!("/jstz_receipt/{}", hash);

    let value = rollup_client.get_value(&key).await?;

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
