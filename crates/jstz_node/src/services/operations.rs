use super::error::{ServiceError, ServiceResult};
use super::{AppState, Service};
use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use jstz_proto::{operation::SignedOperation, receipt::Receipt};
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;

pub struct OperationsService;

async fn inject(
    State(AppState { rollup_client, .. }): State<AppState>,
    Json(operation): Json<SignedOperation>,
) -> ServiceResult<()> {
    let encoded_operation = bincode::serialize(&operation)
        .map_err(|_| anyhow!("Failed to serialize operation"))?;
    let address = rollup_client.get_rollup_address().await?;
    let message_frame = ExternalMessageFrame::Targetted {
        address,
        contents: encoded_operation,
    };
    let mut binary_contents = Vec::new();
    message_frame
        .bin_write(&mut binary_contents)
        .map_err(|_| anyhow!("Failed to write binary frame"))?;
    rollup_client.batcher_injection([binary_contents]).await?;
    Ok(())
}

async fn receipt(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(hash): Path<String>,
) -> ServiceResult<Json<Receipt>> {
    let key = format!("/jstz_receipt/{}", hash);

    let value = rollup_client.get_value(&key).await?;

    let receipt = match value {
        Some(value) => bincode::deserialize::<Receipt>(&value)
            .map_err(|_| anyhow!("Failed to deserialize receipt"))?,
        None => Err(ServiceError::NotFound)?,
    };

    Ok(Json(receipt))
}

impl Service for OperationsService {
    fn router() -> Router<AppState> {
        let routes = Router::new()
            .route("/", post(inject))
            .route("/:operation_hash/receipt", get(receipt));

        Router::new().nest("/operations", routes)
    }
}
