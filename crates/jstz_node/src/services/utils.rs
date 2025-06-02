use crate::{sequencer::db::Db, services::AppState, RunMode};
use anyhow::Context;
use axum::{extract::State, response::IntoResponse};
use octez::OctezRollupClient;
use tezos_crypto_rs::base58::FromBase58Check;

pub async fn get_mode(
    State(AppState { mode, .. }): State<AppState>,
) -> impl IntoResponse {
    serde_json::to_string(&mode).unwrap().into_response()
}

pub(crate) async fn read_value_from_store(
    mode: RunMode,
    rollup_client: OctezRollupClient,
    runtime_db: Db,
    key: String,
) -> anyhow::Result<Option<Vec<u8>>> {
    Ok(match mode {
        RunMode::Default => rollup_client.get_value(&key).await?,
        RunMode::Sequencer => {
            match tokio::task::spawn_blocking(move || runtime_db.read_key(&key))
                .await
                .context("failed to wait for db read task")??
            {
                Some(v) => Some(
                    v.from_base58check()
                        .context("failed to decode value string")?,
                ),
                None => None,
            }
        }
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use jstz_core::BinEncodable;
    use jstz_crypto::{
        hash::Blake2b,
        smart_function_hash::{Kt1Hash, SmartFunctionHash},
    };
    use jstz_proto::receipt::{
        DeployFunctionReceipt, Receipt, ReceiptContent, ReceiptResult,
    };
    use mockito::Matcher;
    use octez::OctezRollupClient;
    use tempfile::NamedTempFile;
    use tezos_crypto_rs::{base58::ToBase58Check, hash::ContractKt1Hash};

    use crate::{sequencer::db::Db, services::utils::read_value_from_store, RunMode};

    pub(crate) fn dummy_receipt(smart_function_hash: ContractKt1Hash) -> Receipt {
        Receipt::new(
            Blake2b::default(),
            Ok(jstz_proto::receipt::ReceiptContent::DeployFunction(
                DeployFunctionReceipt {
                    address: SmartFunctionHash(Kt1Hash(smart_function_hash)),
                },
            )),
        )
    }

    #[tokio::test]
    async fn read_value_from_store_default() {
        let smart_function_hash =
            ContractKt1Hash::from_base58_check("KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX")
                .unwrap();
        let expected = dummy_receipt(smart_function_hash.clone());
        let op_hash = "9b15976cc8162fe39458739de340a1a95c59a9bcff73bd3c83402fad6352396e";
        let mut server = mockito::Server::new_async().await;
        let mock_value_endpoint_ok = server
            .mock("GET", "/global/block/head/durable/wasm_2_0_0/value")
            .match_query(Matcher::UrlEncoded(
                "key".to_string(),
                format!("/jstz_receipt/{op_hash}"),
            ))
            .with_body(format!("\"{}\"", hex::encode(expected.encode().unwrap())))
            .create();
        let mock_value_endpoint_bad = server
            .mock("GET", "/global/block/head/durable/wasm_2_0_0/value")
            .match_query(Matcher::UrlEncoded(
                "key".to_string(),
                "/jstz_receipt/bad_hash".to_string(),
            ))
            .with_body("null")
            .create();
        let runtime_db = Db::init(Some("")).unwrap();

        let bytes = read_value_from_store(
            RunMode::Default,
            OctezRollupClient::new(server.url()),
            runtime_db.clone(),
            format!("/jstz_receipt/{op_hash}"),
        )
        .await
        .expect("should get result from rollup")
        .expect("result should not be none");
        let receipt = Receipt::decode(&bytes).unwrap();
        assert!(matches!(
            receipt.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr == smart_function_hash
        ));

        // non-existent path
        assert!(read_value_from_store(
            RunMode::Default,
            OctezRollupClient::new(server.url()),
            runtime_db.clone(),
            "/jstz_receipt/bad_hash".to_string(),
        )
        .await
        .expect("should get result from rollup")
        .is_none());

        mock_value_endpoint_ok.assert();
        mock_value_endpoint_bad.assert();
    }

    #[tokio::test]
    async fn read_value_from_store_sequencer() {
        let smart_function_hash =
            ContractKt1Hash::from_base58_check("KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX")
                .unwrap();
        let receipt = dummy_receipt(smart_function_hash.clone());
        let op_hash = "9b15976cc8162fe39458739de340a1a95c59a9bcff73bd3c83402fad6352396e";
        let db_file = NamedTempFile::new().unwrap();
        let runtime_db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        runtime_db
            .write(
                &format!("/jstz_receipt/{op_hash}"),
                &receipt.encode().unwrap().to_base58check(),
            )
            .unwrap();
        runtime_db
            .write("/jstz_receipt/bad_value", "nonsense")
            .unwrap();

        // good value
        let bytes = read_value_from_store(
            RunMode::Sequencer,
            OctezRollupClient::new(String::new()),
            runtime_db.clone(),
            format!("/jstz_receipt/{op_hash}"),
        )
        .await
        .expect("should get result from store")
        .expect("result should not be none");
        let receipt = Receipt::decode(&bytes).unwrap();
        assert!(matches!(
            receipt.result,
            ReceiptResult::Success(ReceiptContent::DeployFunction(
                DeployFunctionReceipt { address: SmartFunctionHash(Kt1Hash(addr)) }
            )) if addr == smart_function_hash
        ));

        // bad value
        let error_message = read_value_from_store(
            RunMode::Sequencer,
            OctezRollupClient::new(String::new()),
            runtime_db.clone(),
            "/jstz_receipt/bad_value".to_string(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert_eq!(error_message, "failed to decode value string");

        // non-existent path
        assert!(read_value_from_store(
            RunMode::Sequencer,
            OctezRollupClient::new(String::new()),
            runtime_db.clone(),
            "/jstz_receipt/bad_hash".to_string(),
        )
        .await
        .expect("should get result from store")
        .is_none());
    }
}
