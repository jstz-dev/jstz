use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json,
};
use jstz_api::KvValue;
use jstz_proto::context::account::{Account, Nonce, ParsedCode};
use serde::Deserialize;
use utoipa_axum::router::OpenApiRouter;

use super::{
    error::{ServiceError, ServiceResult},
    Service,
};
use crate::AppState;

fn construct_storage_key(address: &str, key: &Option<String>) -> String {
    match key {
        Some(value) if !value.is_empty() => format!("/jstz_kv/{}/{}", address, value),
        _ => format!("/jstz_kv/{}", address),
    }
}

#[derive(Deserialize)]
struct KvQuery {
    key: Option<String>,
}

pub struct AccountsService;

async fn nonce(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<Nonce>> {
    let key = format!("/jstz_account/{}", address);
    let value = rollup_client.get_value(&key).await?;
    let account_nonce = match value {
        Some(value) => {
            bincode::deserialize::<Account>(&value)
                .map_err(|_| anyhow!("Failed to deserialize account"))?
                .nonce
        }
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account_nonce))
}

async fn code(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<ParsedCode>> {
    let key = format!("/jstz_account/{}", address);
    let value = rollup_client.get_value(&key).await?;
    let account_code = match value {
        Some(value) => {
            bincode::deserialize::<Account>(&value)
                .map_err(|_| anyhow!("Failed to deserialize account"))?
                .function_code
        }
        None => Err(ServiceError::NotFound)?,
    }
    .ok_or_else(|| {
        ServiceError::BadRequest("Account is not a smart function".to_string())
    })?;
    Ok(Json(account_code))
}

async fn balance(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<u64>> {
    let key = format!("/jstz_account/{}", address);
    let value = rollup_client.get_value(&key).await?;
    let account_balance = match value {
        Some(value) => {
            bincode::deserialize::<Account>(&value)
                .map_err(|_| anyhow!("Failed to deserialize account"))?
                .amount
        }
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account_balance))
}

async fn kv(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
    Query(KvQuery { key }): Query<KvQuery>,
) -> ServiceResult<Json<KvValue>> {
    let key = construct_storage_key(&address, &key);
    let value = rollup_client.get_value(&key).await?;
    let kv_value = match value {
        Some(value) => bincode::deserialize::<KvValue>(&value)
            .map_err(|_| anyhow!("Failed to deserialize account"))?,
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(kv_value))
}

async fn kv_subkeys(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
    Query(KvQuery { key }): Query<KvQuery>,
) -> ServiceResult<Json<Vec<String>>> {
    let key = construct_storage_key(&address, &key);
    let value = rollup_client.get_subkeys(&key).await?;
    let subkeys = match value {
        Some(value) => value,
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(subkeys))
}

impl Service for AccountsService {
    fn router_with_openapi() -> OpenApiRouter<AppState> {
        let routes = OpenApiRouter::new()
            .route("/:address/nonce", get(nonce))
            .route("/:address/code", get(code))
            .route("/:address/balance", get(balance))
            .route("/:address/kv", get(kv))
            .route("/:address/kv/subkeys", get(kv_subkeys));

        OpenApiRouter::new().nest("/accounts", routes)
    }
}
