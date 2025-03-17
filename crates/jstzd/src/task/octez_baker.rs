use super::{
    child_wrapper::{ChildWrapper, SharedChildWrapper},
    Task,
};
use anyhow::Result;
use async_trait::async_trait;
use octez::r#async::baker::{self, OctezBakerConfig};

#[allow(dead_code)]
pub struct OctezBaker {
    inner: SharedChildWrapper,
}

#[async_trait]
impl Task for OctezBaker {
    type Config = OctezBakerConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let child = baker::OctezBaker::run(config).await?;
        let inner = ChildWrapper::new_shared(child);
        Ok(OctezBaker { inner })
    }

    async fn kill(&mut self) -> Result<()> {
        let mut lock = self.inner.write().await;
        lock.kill().await
    }

    async fn health_check(&self) -> Result<bool> {
        let mut lock = self.inner.write().await;
        Ok(lock.inner_mut().is_running().await)
    }
}
