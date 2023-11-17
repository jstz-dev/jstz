use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use http::StatusCode;
use jstz_api::KvValue;
use jstz_proto::{context::account::Nonce, operation::OperationHash, receipt::Receipt};
use tokio::time::sleep;

use crate::config::Config;

pub struct JstzClient {
    endpoint: String,
    client: reqwest::Client,
}

impl JstzClient {
    pub fn new(cfg: &Config) -> Self {
        Self {
            endpoint: format!("http://127.0.0.1:{}", cfg.jstz_node_port),
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_operation_receipt(
        &self,
        hash: &OperationHash,
    ) -> Result<Option<Receipt>> {
        let response = self
            .get(&format!(
                "{}/operations/{}/receipt",
                self.endpoint,
                hash.to_string()
            ))
            .await?;

        if response.status().is_success() {
            let receipt = response.json::<Receipt>().await?;

            Ok(Some(receipt))
        } else {
            Ok(None)
        }
    }

    pub async fn get_nonce(&self, address: &str) -> Result<Nonce> {
        let response = self
            .get(&format!("{}/accounts/{}/nonce", self.endpoint, address))
            .await?;

        match response.status() {
            StatusCode::OK => {
                let nonce = response.json::<Nonce>().await?;
                Ok(nonce)
            }
            StatusCode::NOT_FOUND => Ok(Nonce::default()),
            // For any other status, return a generic error
            _ => Err(anyhow!("Failed to get nonce")),
        }
    }

    pub async fn get_value(&self, address: &str, key: &str) -> Result<Option<KvValue>> {
        let response = self
            .get(&format!(
                "{}/accounts/{}/kv?key={}",
                self.endpoint, address, key
            ))
            .await?;

        match response.status() {
            StatusCode::OK => {
                let kv = response.json::<KvValue>().await?;
                Ok(Some(kv))
            }
            StatusCode::NOT_FOUND => Ok(None),
            // For any other status, return a generic error
            _ => Err(anyhow!("Failed to get value.")),
        }
    }

    pub async fn get_subkey_list(
        &self,
        address: &str,
        key: &str,
    ) -> Result<Option<Vec<String>>> {
        let response = self
            .get(&format!(
                "{}/accounts/{}/kv/subkeys?key={}",
                self.endpoint, address, key
            ))
            .await?;

        match response.status() {
            StatusCode::OK => {
                let kv = response.json::<Vec<String>>().await?;
                Ok(Some(kv))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(anyhow!("Failed to get subkey list.")),
        }
    }

    pub async fn wait_for_operation_receipt(
        &self,
        hash: &OperationHash,
    ) -> Result<Receipt> {
        loop {
            if let Some(receipt) = self.get_operation_receipt(hash).await? {
                return Ok(receipt);
            }

            // tokio sleep
            sleep(Duration::from_millis(100)).await;
        }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response> {
        Ok(self.client.get(url).send().await?)
    }
}
