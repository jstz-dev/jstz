use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use jstz_core::BinEncodable;
use jstz_proto::{
    context::account::{
        Account, Nonce, SmartFunctionAccount, UserAccount, ACCOUNTS_PATH_PREFIX,
    },
    runtime::{KvValue, ParsedCode},
};
use octez::OctezRollupClient;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{
    error::{ServiceError, ServiceResult},
    Service,
};
use crate::{sequencer::db::Db, utils::read_value_from_store, AppState, RunMode};

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
    State(AppState {
        mode,
        rollup_client,
        runtime_db,
        ..
    }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<Account>> {
    let key = format!("/jstz_account/{}", address);
    let value = read_value_from_store(mode, rollup_client, runtime_db, key).await?;
    let account = match value {
        Some(value) => deserialize_account(value.as_slice())?,
        None => Err(ServiceError::NotFound)?,
    };
    Ok(Json(account))
}

// FIXME: This will be cleaned up in JSTZ-592.
pub(crate) async fn get_account_nonce(
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

async fn get_account_nonce_from_store(
    mode: RunMode,
    rollup_client: OctezRollupClient,
    runtime_db: Db,
    address: &str,
) -> ServiceResult<Option<Nonce>> {
    let key = construct_accounts_key(address);
    let value = read_value_from_store(mode, rollup_client, runtime_db, key).await?;
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
    State(AppState {
        mode,
        rollup_client,
        runtime_db,
        ..
    }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<Nonce>> {
    let account_nonce =
        get_account_nonce_from_store(mode, rollup_client, runtime_db, &address).await?;
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
    State(AppState {
        mode,
        rollup_client,
        runtime_db,
        ..
    }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<ParsedCode>> {
    let key = construct_accounts_key(&address);
    let value = read_value_from_store(mode, rollup_client, runtime_db, key).await?;
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
    State(AppState {
        mode,
        rollup_client,
        runtime_db,
        ..
    }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Json<u64>> {
    let key = construct_accounts_key(&address);
    let value = read_value_from_store(mode, rollup_client, runtime_db, key).await?;
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
    State(AppState {
        mode,
        rollup_client,
        runtime_db,
        ..
    }): State<AppState>,
    Path(address): Path<String>,
    Query(KvQuery { key }): Query<KvQuery>,
) -> ServiceResult<Json<KvValue>> {
    let key = construct_storage_key(&address, &key);
    let value = read_value_from_store(mode, rollup_client, runtime_db, key).await?;
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

#[cfg(test)]
mod tests {
    use std::{borrow::BorrowMut, convert::Infallible};

    use axum::{body::Body, extract::Request, response::Response, Router};
    use jstz_core::BinEncodable;
    use jstz_proto::{
        context::account::{Account, Nonce, SmartFunctionAccount, UserAccount},
        runtime::{KvValue, ParsedCode},
    };
    use mockito::Matcher;
    use octez::OctezRollupClient;
    use tempfile::NamedTempFile;
    use tezos_crypto_rs::base58::ToBase58Check;
    use tower::ServiceExt;

    use crate::{
        services::{accounts::AccountsService, Service},
        utils::tests::mock_app_state,
        RunMode,
    };

    async fn send_simple_get_request<S: Into<String>>(
        router: &mut Router,
        uri: S,
    ) -> Result<Response, Infallible> {
        router
            .oneshot(
                Request::builder()
                    .uri(uri.into())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
    }

    #[tokio::test]
    async fn get_account_sequencer() {
        let expected = Account::User(UserAccount {
            amount: 300,
            nonce: Nonce(1),
        });
        let addr = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let db_file = NamedTempFile::new().unwrap();
        let state =
            mock_app_state("", db_file.path().to_str().unwrap(), RunMode::Sequencer)
                .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{addr}"),
                &expected.encode().unwrap().to_base58check(),
            )
            .unwrap();

        let (mut router, _) = AccountsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let res =
            send_simple_get_request(router.borrow_mut(), format!("/accounts/{addr}"))
                .await
                .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let account = serde_json::from_slice::<Account>(&bytes).unwrap();
        assert!(matches!(
            account,
            Account::User(UserAccount {
                amount: 300,
                nonce: Nonce(1),
            })
        ));

        // non-existent address
        let res = send_simple_get_request(router.borrow_mut(), "/accounts/bad_addr")
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn get_account_nonce_from_store() {
        let user_account = Account::User(UserAccount {
            amount: 0,
            nonce: Nonce(42),
        });
        let user_account_hash = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let smart_function_account = Account::SmartFunction(SmartFunctionAccount {
            amount: 0,
            nonce: Nonce(50),
            function_code: ParsedCode::default(),
        });
        let smart_function_hash = "KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX";
        let mut server = mockito::Server::new_async().await;
        let mock_value_endpoint_user = server
            .mock("GET", "/global/block/head/durable/wasm_2_0_0/value")
            .match_query(Matcher::UrlEncoded(
                "key".to_string(),
                format!("/jstz_account/{user_account_hash}"),
            ))
            .with_body(format!(
                "\"{}\"",
                hex::encode(user_account.encode().unwrap())
            ))
            .create();
        let mock_value_endpoint_smart_function = server
            .mock("GET", "/global/block/head/durable/wasm_2_0_0/value")
            .match_query(Matcher::UrlEncoded(
                "key".to_string(),
                format!("/jstz_account/{smart_function_hash}"),
            ))
            .with_body(format!(
                "\"{}\"",
                hex::encode(smart_function_account.encode().unwrap())
            ))
            .create();
        let mock_value_endpoint_bad = server
            .mock("GET", "/global/block/head/durable/wasm_2_0_0/value")
            .match_query(Matcher::UrlEncoded(
                "key".to_string(),
                "/jstz_account/bad_hash".to_string(),
            ))
            .with_body("null")
            .create();

        assert!(super::get_account_nonce_from_store(
            RunMode::Default,
            OctezRollupClient::new(server.url()),
            crate::sequencer::db::Db::init(Some("")).unwrap(),
            user_account_hash,
        )
        .await
        .is_ok_and(|v| matches!(v.unwrap(), Nonce(42))));

        assert!(super::get_account_nonce_from_store(
            RunMode::Default,
            OctezRollupClient::new(server.url()),
            crate::sequencer::db::Db::init(Some("")).unwrap(),
            smart_function_hash,
        )
        .await
        .is_ok_and(|v| matches!(v.unwrap(), Nonce(50))));

        assert!(super::get_account_nonce_from_store(
            RunMode::Default,
            OctezRollupClient::new(server.url()),
            crate::sequencer::db::Db::init(Some("")).unwrap(),
            "bad_hash",
        )
        .await
        .is_ok_and(|v| v.is_none()));

        mock_value_endpoint_user.assert();
        mock_value_endpoint_smart_function.assert();
        mock_value_endpoint_bad.assert();
    }

    #[tokio::test]
    async fn get_nonce_sequencer() {
        let account = Account::User(UserAccount {
            amount: 0,
            nonce: Nonce(42),
        });
        let addr = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let db_file = NamedTempFile::new().unwrap();
        let state =
            mock_app_state("", db_file.path().to_str().unwrap(), RunMode::Sequencer)
                .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{addr}"),
                &account.encode().unwrap().to_base58check(),
            )
            .unwrap();

        let (mut router, _) = AccountsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{addr}/nonce"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let nonce = serde_json::from_slice::<Nonce>(&bytes).unwrap();
        assert!(matches!(nonce, Nonce(42)));

        // non-existent address
        let res =
            send_simple_get_request(router.borrow_mut(), "/accounts/bad_addr/nonce")
                .await
                .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn get_code_sequencer() {
        let user_account = Account::User(UserAccount {
            amount: 0,
            nonce: Nonce(42),
        });
        let user_account_hash = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let smart_function_account = Account::SmartFunction(SmartFunctionAccount {
            amount: 0,
            nonce: Nonce(50),
            function_code: ParsedCode("dummy_code".to_string()),
        });
        let smart_function_hash = "KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX";
        let db_file = NamedTempFile::new().unwrap();
        let state =
            mock_app_state("", db_file.path().to_str().unwrap(), RunMode::Sequencer)
                .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{smart_function_hash}"),
                &smart_function_account.encode().unwrap().to_base58check(),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{user_account_hash}"),
                &user_account.encode().unwrap().to_base58check(),
            )
            .unwrap();

        let (mut router, _) = AccountsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{smart_function_hash}/code"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let code = serde_json::from_slice::<String>(&bytes).unwrap();
        assert_eq!(code, "dummy_code");

        // non-smart function address
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{user_account_hash}/code"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 400);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let error_message = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        assert_eq!(
            error_message,
            serde_json::json!({"error": "Account is not a smart function"})
        );

        // non-existent address
        let res =
            send_simple_get_request(router.borrow_mut(), "/accounts/bad_address/code")
                .await
                .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn get_balance_sequencer() {
        let user_account = Account::User(UserAccount {
            amount: 999,
            nonce: Nonce(42),
        });
        let user_account_hash = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let smart_function_account = Account::SmartFunction(SmartFunctionAccount {
            amount: 888,
            nonce: Nonce(50),
            function_code: ParsedCode::default(),
        });
        let smart_function_hash = "KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX";
        let db_file = NamedTempFile::new().unwrap();
        let state =
            mock_app_state("", db_file.path().to_str().unwrap(), RunMode::Sequencer)
                .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{smart_function_hash}"),
                &smart_function_account.encode().unwrap().to_base58check(),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                &format!("/jstz_account/{user_account_hash}"),
                &user_account.encode().unwrap().to_base58check(),
            )
            .unwrap();

        let (mut router, _) = AccountsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();

        // user account
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{user_account_hash}/balance"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let balance = serde_json::from_slice::<u64>(&bytes).unwrap();
        assert_eq!(balance, 999);

        // smart function account
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{smart_function_hash}/balance"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let balance = serde_json::from_slice::<u64>(&bytes).unwrap();
        assert_eq!(balance, 888);

        // non-existent address
        let res =
            send_simple_get_request(router.borrow_mut(), "/accounts/bad_addr/nonce")
                .await
                .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn get_kv_value_sequencer() {
        let address = "tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV";
        let db_file = NamedTempFile::new().unwrap();
        let state =
            mock_app_state("", db_file.path().to_str().unwrap(), RunMode::Sequencer)
                .await;
        state
            .runtime_db
            .write(
                &format!("/jstz_kv/{address}/foo"),
                &KvValue(serde_json::json!("foo!"))
                    .encode()
                    .unwrap()
                    .to_base58check(),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                &format!("/jstz_kv/{address}/foo/bar"),
                &KvValue(serde_json::json!({"bar": "bar!"}))
                    .encode()
                    .unwrap()
                    .to_base58check(),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                &format!("/jstz_kv/{address}/bad_value"),
                &[6, 0, 0, 0, 0, 0, 0, 0, 34].to_base58check(),
            )
            .unwrap();
        state
            .runtime_db
            .write(
                &format!("/jstz_kv/{address}"),
                &KvValue(serde_json::json!("root!"))
                    .encode()
                    .unwrap()
                    .to_base58check(),
            )
            .unwrap();

        let (mut router, _) = AccountsService::router_with_openapi()
            .with_state(state)
            .split_for_parts();

        // root level
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{address}/kv"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let value = serde_json::from_slice::<KvValue>(&bytes).unwrap();
        assert_eq!(value.0, serde_json::json!("root!"));

        // base level key
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{address}/kv?key=foo"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let value = serde_json::from_slice::<KvValue>(&bytes).unwrap();
        assert_eq!(value.0, serde_json::json!("foo!"));

        // nested key
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{address}/kv?key=foo/bar"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let value = serde_json::from_slice::<KvValue>(&bytes).unwrap();
        assert_eq!(value.0, serde_json::json!({"bar": "bar!"}));

        // bad non-json value
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{address}/kv?key=bad_value"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 500);
        let bytes = axum::body::to_bytes(res.into_body(), 1000).await.unwrap();
        let error_message = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        assert_eq!(
            error_message,
            serde_json::json!({"error": "Failed to deserialize kv value"})
        );

        // non-existent key
        let res = send_simple_get_request(
            router.borrow_mut(),
            format!("/accounts/{address}/kv?key=nonexistent_key"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), 404);
    }
}
