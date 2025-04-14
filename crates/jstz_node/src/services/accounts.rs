use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use jstz_core::BinEncodable;
use jstz_proto::{
    api::KvValue,
    context::account::{
        Account, Nonce, ParsedCode, SmartFunctionAccount, UserAccount,
        ACCOUNTS_PATH_PREFIX,
    },
};
use octez::OctezRollupClient;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{
    error::{ServiceError, ServiceResult},
    Service,
};
use crate::AppState;

const ACCOUNTS_TAG: &str = "Accounts";

fn construct_storage_key(address: &str, key: &Option<String>) -> String {
    match key {
        Some(value) if !value.is_empty() => format!("/jstz_kv/{}/{}", address, value),
        _ => format!("/jstz_kv/{}", address),
    }
}

fn deserialize_account(data: &[u8]) -> ServiceResult<Account> {
    Ok(Account::decode(data).map_err(|_| anyhow!("Failed to deserialize account"))?)
}

fn construct_accounts_key(address: &str) -> String {
    format!("{}/{}", ACCOUNTS_PATH_PREFIX, address)
}

#[derive(Deserialize, IntoParams)]
struct KvQuery {
    key: Option<String>,
}

pub struct AccountsService;

/// Get account
#[utoipa::path(
    get,
    path = "/{address}",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = Account),
        (status = 404),
        (status = 500)
    )
)]
async fn get_account(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<Account>> {
    let key = format!("/jstz_account/{}", address);
    let value = rollup_client.get_value(&key).await?;
    let account = match value {
        Some(value) => deserialize_account(value.as_slice())?,
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account))
}

pub async fn get_account_nonce(
    rollup_client: &OctezRollupClient,
    address: &str,
) -> ServiceResult<Option<Nonce>> {
    let key = construct_accounts_key(address);
    let value = rollup_client.get_value(&key).await?;
    match value {
        Some(value) => match deserialize_account(value.as_slice())? {
            Account::User(UserAccount { nonce, .. }) => Ok(Some(nonce)),
            Account::SmartFunction(SmartFunctionAccount { nonce, .. }) => Ok(Some(nonce)),
        },
        None => Ok(None),
    }
}

/// Get nonce of an account
#[utoipa::path(
    get,
    path = "/{address}/nonce",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = Nonce),
        (status = 404),
        (status = 500)
    )
)]
async fn get_nonce(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<Nonce>> {
    let account_nonce = get_account_nonce(&rollup_client, &address).await?;
    match account_nonce {
        Some(nonce) => Ok(Json(nonce)),
        None => Err(ServiceError::NotFound)?,
    }
}

/// Get code of an account
#[utoipa::path(
    get,
    path = "/{address}/code",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = ParsedCode),
        (status = 400),
        (status = 404),
        (status = 500)
    )
)]
async fn get_code(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<ParsedCode>> {
    let key = construct_accounts_key(&address);
    let value = rollup_client.get_value(&key).await?;
    let account_code = match value {
        Some(value) => {
            if let Account::SmartFunction(SmartFunctionAccount {
                function_code, ..
            }) = deserialize_account(value.as_slice())?
            {
                function_code
            } else {
                Err(ServiceError::BadRequest(
                    "Account is not a smart function".to_string(),
                ))?
            }
        }
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account_code))
}

/// Get balance of an account
#[utoipa::path(
    get,
    path = "/{address}/balance",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = u64),
        (status = 404),
        (status = 500)
    )
)]
async fn get_balance(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<u64>> {
    let key = construct_accounts_key(&address);
    let value = rollup_client.get_value(&key).await?;
    let account_balance = match value {
        Some(value) => match deserialize_account(value.as_slice())? {
            Account::User(UserAccount { amount, .. }) => amount,
            Account::SmartFunction(SmartFunctionAccount { amount, .. }) => amount,
        },
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account_balance))
}

/// Get KV value under a given key path
///
/// Get KV value under a given key path for an account. If `key` is not provided,
/// the empty key path will be used.
#[utoipa::path(
    get,
    params(KvQuery),
    path = "/{address}/kv",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = KvValue),
        (status = 404),
        (status = 500)
    )
)]
async fn get_kv_value(
    State(AppState { rollup_client, .. }): State<AppState>,
    Path(address): Path<String>,
    Query(KvQuery { key }): Query<KvQuery>,
) -> ServiceResult<Json<KvValue>> {
    let key = construct_storage_key(&address, &key);
    let value = rollup_client.get_value(&key).await?;
    let kv_value = match value {
        Some(value) => KvValue::decode(value.as_slice())
            .map_err(|_| anyhow!("Failed to deserialize kv value"))?,
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(kv_value))
}

/// Get array of KV subkeys under a given key path
///
/// Get array of KV subkeys under a given key path for an account. If `key` is not provided,
/// the empty key path will be used.
#[utoipa::path(
    get,
    params(KvQuery),
    path = "/{address}/kv/subkeys",
    tag = ACCOUNTS_TAG,
    responses(
        (status = 200, body = Vec<String>),
        (status = 404),
        (status = 500)
    )
)]
async fn get_kv_subkeys(
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
            .routes(routes!(get_account))
            .routes(routes!(get_nonce))
            .routes(routes!(get_code))
            .routes(routes!(get_balance))
            .routes(routes!(get_kv_value))
            .routes(routes!(get_kv_subkeys));

        OpenApiRouter::new().nest("/accounts", routes)
    }
}
