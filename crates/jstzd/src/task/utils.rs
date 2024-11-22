use anyhow::{anyhow, Result};
use serde_json::Value;

pub async fn retry<'a, F>(retries: u16, interval_ms: u64, f: impl Fn() -> F) -> bool
where
    F: std::future::Future<Output = anyhow::Result<bool>> + Send + 'a,
{
    let duration = tokio::time::Duration::from_millis(interval_ms);
    for _ in 0..retries {
        tokio::time::sleep(duration).await;
        if let Ok(v) = f().await {
            if v {
                return true;
            }
        }
    }
    false
}

pub async fn get_block_level(rpc_endpoint: &str) -> Result<i64> {
    let blocks_head_endpoint = format!("{}/chains/main/blocks/head", rpc_endpoint);
    let response: Value = reqwest::get(&blocks_head_endpoint).await?.json().await?;

    let level = response
        .get("header")
        .and_then(|header| header.get("level"))
        .ok_or_else(|| anyhow!("Failed to extract level from head block"))?;
    level
        .as_i64()
        .ok_or_else(|| anyhow!("Level is not a valid i64"))
}
