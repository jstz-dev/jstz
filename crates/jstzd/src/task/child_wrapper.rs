use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

pub type SharedChildWrapper = Arc<RwLock<AsyncDropper<ChildWrapper>>>;

#[derive(Default)]
pub struct ChildWrapper {
    inner: Option<Child>,
}

impl ChildWrapper {
    fn new(child: Child) -> Self {
        Self { inner: Some(child) }
    }

    pub fn new_shared(child: Child) -> SharedChildWrapper {
        Arc::new(RwLock::new(AsyncDropper::new(Self::new(child))))
    }

    pub async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(mut v) = self.inner.take() {
            v.kill().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncDrop for ChildWrapper {
    async fn async_drop(&mut self) {
        let _ = self.kill().await;
    }
}
