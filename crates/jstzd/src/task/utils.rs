use anyhow::{anyhow, Result};
use serde_json::Value;

pub async fn retry<'a, F>(
    retries: u16,
    interval_ms: u64,
    f: impl Fn() -> F + Sync,
) -> bool
where
    F: std::future::Future<Output = anyhow::Result<bool>> + Send + 'a,
{
    poll(retries, interval_ms, || async {
        if let Ok(v) = f().await {
            if v {
                return Some(true);
            }
        }
        None
    })
    .await
    .unwrap_or(false)
}

pub async fn poll<'a, F, T>(
    max_attempts: u16,
    interval_ms: u64,
    f: impl Fn() -> F,
) -> Option<T>
where
    F: std::future::Future<Output = Option<T>> + Send + 'a,
{
    let duration = tokio::time::Duration::from_millis(interval_ms);
    for _ in 0..max_attempts {
        tokio::time::sleep(duration).await;
        if let Some(v) = f().await {
            return Some(v);
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn retry() {
        async fn check(locked: Arc<Mutex<i32>>, result: bool) -> anyhow::Result<bool> {
            let mut v = locked.lock().await;
            if *v == 5 {
                return Ok(result);
            }
            *v += 1;
            Err(anyhow::anyhow!(""))
        }

        // retry till the end and get a positive result
        let locked = Arc::new(Mutex::new(1));
        assert!(super::retry(5, 1, || async { check(locked.clone(), true).await }).await);

        // retry till the end and get a negative result
        let locked = Arc::new(Mutex::new(1));
        assert!(
            !super::retry(5, 1, || async { check(locked.clone(), false).await }).await
        );

        // not waiting long enough
        let locked = Arc::new(Mutex::new(1));
        assert!(
            !super::retry(2, 1, || async { check(locked.clone(), true).await }).await
        );
    }

    #[tokio::test]
    async fn poll() {
        async fn check(locked: Arc<Mutex<i32>>, result: bool) -> Option<bool> {
            let mut v = locked.lock().await;
            if *v == 5 {
                return Some(result);
            }
            *v += 1;
            None
        }

        // poll till the end and get a positive result
        let locked = Arc::new(Mutex::new(1));
        assert!(
            super::poll(5, 1, || async { check(locked.clone(), true).await })
                .await
                .unwrap()
        );

        // poll till the end and get a negative result
        let locked = Arc::new(Mutex::new(1));
        assert!(
            !super::poll(5, 1, || async { check(locked.clone(), false).await })
                .await
                .unwrap()
        );

        // not waiting long enough
        let locked = Arc::new(Mutex::new(1));
        assert!(
            super::poll(2, 1, || async { check(locked.clone(), true).await })
                .await
                .is_none()
        );
    }
}
