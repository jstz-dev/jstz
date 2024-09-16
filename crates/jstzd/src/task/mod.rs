use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Task: Sized {
    type Config;

    /// Spins up the task with the given config.
    async fn spawn(config: Self::Config) -> Result<Self>;

    /// Aborts the running task.
    async fn kill(&mut self) -> Result<()>;

    /// Conducts a health check on the running task.
    async fn health_check(&self) -> Result<bool>;
}
