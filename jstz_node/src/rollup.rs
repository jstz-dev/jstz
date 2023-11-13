use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

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
#[derive(Debug)]
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

    pub async fn get_subkeys(&self, key: &str) -> Result<Option<Vec<String>>> {
        let res = self
            .client
            .get(format!(
                "{}/global/block/head/durable/wasm_2_0_0/subkeys?key={}",
                self.endpoint, key
            ))
            .send()
            .await?;

        if res.status() == 200 || res.status() == 500 {
            let content_str = res.text().await?;
            let content_json = serde_json::from_str::<Value>(&content_str);

            match content_json {
                Ok(serde_json::Value::Array(arr)) => {
                    let list_of_strings: Result<Vec<String>> = arr
                        .into_iter()
                        .map(|item| match item {
                            Value::String(s) => Ok(s),
                            _ => Err(anyhow!("Non-string element found in the array")),
                        })
                        .collect();

                    match list_of_strings {
                        Ok(list) => Ok(Some(list)),
                        Err(e) => Err(e),
                    }
                }
                Ok(_) => Err(anyhow!(
                    "Expected a JSON array but got a different structure"
                )),
                Err(e) => Err(anyhow!("Failed to parse content as JSON: {:?}", e)),
            }
        } else {
            Err(anyhow!("Unhandled response status: {}", res.status()))
        }
    }
}
