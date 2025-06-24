use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

pub type Shared<T> = Arc<RwLock<T>>;

pub type SharedChildWrapper = Shared<AsyncDropper<ChildWrapper>>;

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
    /// Check if the child process is running
    /// mutable borrow because the process id could be reaped if it exited
    pub async fn is_running(&mut self) -> bool {
        self.inner
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }
}

#[async_trait]
impl AsyncDrop for ChildWrapper {
    async fn async_drop(&mut self) {
        let _ = self.kill().await;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_child() {
        let child = tokio::process::Command::new("sleep")
            .arg("1")
            .spawn()
            .unwrap();
        let wrapper = ChildWrapper::new_shared(child);
        assert!(wrapper.write().await.is_running().await);
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        assert!(!wrapper.write().await.is_running().await);
    }
}
