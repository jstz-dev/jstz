use std::time::Duration;

use anyhow::{bail, Result};
use jstz_api::KvValue;
use jstz_node::logs::{QueryParams, QueryResponse};
use jstz_proto::{
    context::account::{Address, Nonce},
    operation::{OperationHash, SignedOperation},
    receipt::Receipt,
};
use reqwest::StatusCode;
use reqwest_eventsource::EventSource;
use tokio::time::sleep;

pub struct JstzClient {
    endpoint: String,
    client: reqwest::Client,
}

impl JstzClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    pub fn logs_stream(&self, address: &Address) -> EventSource {
        let url = format!("{}/logs/{}/stream", self.endpoint, address);
        EventSource::get(url)
    }

    pub async fn logs_persistnet(&self, params: QueryParams) -> Result<QueryResponse> {
        let url = match params {
            QueryParams::GetLogsByAddressAndRequestId(address, request_id) => format!(
                "{}/logs/{}/persistent/requests/{}",
                self.endpoint, address, request_id
            ),
            QueryParams::GetLogsByAddress(address, limit, offset) => {
                let base_url =
                    format!("{}/logs/{}/persistent/requests", self.endpoint, address);
                match (limit, offset) {
                    (Some(limit), Some(offset)) => {
                        base_url + &format!("?limit={}&offset={}", limit, offset)
                    }
                    (Some(limit), _) => base_url + &format!("?limit={}", limit),
                    (_, Some(offset)) => base_url + &format!("?offset={}", offset),
                    _ => base_url,
                }
            }
        };

        let response = self.get(&url).await?;

        match response.status() {
            StatusCode::OK => {
                let logs = response.json::<QueryResponse>().await?;
                Ok(logs)
            }
            _ => bail!("Failed to get logs."),
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

    pub async fn get_nonce(&self, address: &Address) -> Result<Nonce> {
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
            _ => bail!("Failed to get nonce"),
        }
    }

    pub async fn get_code(&self, address: &Address) -> Result<Option<String>> {
        let response = self
            .get(&format!("{}/accounts/{}/code", self.endpoint, address))
            .await?;

        match response.status() {
            StatusCode::OK => {
                let code = response.json::<Option<String>>().await?;
                Ok(code)
            }
            // For any other status, return a generic error
            _ => bail!("Failed to get the code"),
        }
    }

    pub async fn get_balance(&self, address: &Address) -> Result<u64> {
        let response = self
            .get(&format!("{}/accounts/{}/balance", self.endpoint, address))
            .await?;

        match response.status() {
            StatusCode::OK => {
                let balance = response.json::<u64>().await?;
                Ok(balance)
            }
            _ => bail!("Failed to get the balance"),
        }
    }

    pub async fn get_value(
        &self,
        address: &Address,
        key: &str,
    ) -> Result<Option<KvValue>> {
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
            _ => bail!("Failed to get value."),
        }
    }

    pub async fn get_subkey_list(
        &self,
        address: &Address,
        key: &Option<String>,
    ) -> Result<Option<Vec<String>>> {
        let url = match key {
            Some(k) if !k.is_empty() => format!(
                "{}/accounts/{}/kv/subkeys?key={}",
                self.endpoint, address, k
            ),
            _ => format!("{}/accounts/{}/kv/subkeys", self.endpoint, address),
        };

        let response = self.get(&url).await?;

        match response.status() {
            StatusCode::OK => {
                let kv = response.json::<Vec<String>>().await?;
                Ok(Some(kv))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => bail!("Failed to get subkey list."),
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
            sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn post_operation(&self, operation: &SignedOperation) -> Result<()> {
        let response = self
            .client
            .post(&format!("{}/operations", self.endpoint))
            .body(bincode::serialize(operation)?)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(()),
            // For any other status, return a generic error
            _ => bail!("Failed to post operation"),
        }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response> {
        Ok(self.client.get(url).send().await?)
    }
}
