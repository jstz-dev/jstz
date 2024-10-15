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

#[allow(dead_code)]
pub async fn get_request(endpoint: &str) -> String {
    reqwest::get(endpoint)
        .await
        .expect("Failed to get block head")
        .text()
        .await
        .expect("Failed to get response text")
}
