use super::Task;
use anyhow::Result;
use async_trait::async_trait;
use jstz_node::{config::JstzNodeConfig, run_with_config};
use tokio::task::JoinHandle;

pub struct JstzNode {
    handle: JoinHandle<Result<()>>,
    config: JstzNodeConfig,
}

#[async_trait]
impl Task for JstzNode {
    type Config = JstzNodeConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let cfg = config.clone();
        let handle = tokio::spawn(async move { run_with_config(cfg).await });
        Ok(JstzNode { handle, config })
    }

    async fn kill(&mut self) -> Result<()> {
        self.handle.abort();
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let res = reqwest::get(format!("{}/health", self.config.endpoint)).await;
        Ok(res.is_ok_and(|res| res.status().is_success()))
    }
}
