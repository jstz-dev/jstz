use std::time::Duration;

use anyhow::Result;
use jstz_proto::{operation::OperationHash, receipt::Receipt};
use tokio::time::sleep;

pub struct JstzNode {
    endpoint: String,
    client: reqwest::Client,
}

impl JstzNode {
    pub fn new() -> Self {
        Self {
            endpoint: "http://localhost:8933".to_string(),
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
