use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug)]
pub struct RollupClient {
    endpoint: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct ValueError {
    pub kind: String,
    pub id: String,
    pub block: Option<String>,
    pub msg: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ValueResponse {
    Value(String),
    Errors(Vec<ValueError>),
}

impl RollupClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_value(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let res = self
            .client
            .get(format!(
                "{}/global/block/head/durable/wasm_2_0_0/value?key={}",
                self.endpoint, key
            ))
            .send()
            .await?;

        if res.status() == 200 || res.status() == 500 {
            let content: Option<ValueResponse> = res.json().await?;
            match content {
                Some(ValueResponse::Value(value)) => {
                    let payload = hex::decode(value)?;
                    Ok(Some(payload))
                }
                Some(ValueResponse::Errors(errors)) => Err(anyhow!(
                    "Failed to get value of key-value pair: {}. Errors: {:?}",
                    key,
                    errors
                )),
                None => Ok(None),
            }
        } else {
            Err(anyhow!("Unhandled response status: {}", res.status()))
        }
    }
}
