use anyhow::Result;
use async_dropper::AsyncDrop;
use async_scoped::TokioScope;
use async_trait::async_trait;
use bollard::Docker;
use log::error;
use std::sync::Arc;

pub struct Container {
    pub id: String,
    client: Option<Arc<Docker>>,
    dropped: bool,
    _private: (),
}

impl Default for Container {
    fn default() -> Self {
        Self {
            id: String::new(),
            client: None,
            dropped: false,
            _private: (),
        }
    }
}

impl Container {
    /// Creates a new container with running `id`
    pub(super) fn new(client: Arc<Docker>, id: String) -> Self {
        Self {
            id,
            client: Some(client),
            dropped: false,
            _private: (),
        }
    }

    // Starts the container's entrypoint
    pub async fn start(&self) -> Result<()> {
        self.client()?
            .start_container::<String>(&self.id, None)
            .await?;
        Ok(())
    }

    // Stop the container
    pub async fn stop(&self) -> Result<()> {
        self.client()?.stop_container(&self.id, None).await?;
        Ok(())
    }

    // Remove the container
    pub async fn remove(&self) -> Result<()> {
        match self.client()?.remove_container(&self.id, None).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("404") {
                    return Err(anyhow::anyhow!(
                        "Failed to remove non existent container: {}",
                        self.id
                    ));
                }
                Err(e.into())
            }
        }
    }

    fn client(&self) -> Result<&Arc<Docker>> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Client does not exist"))
    }
}

#[async_trait]
impl AsyncDrop for Container {
    async fn async_drop(&mut self) {
        self.stop().await.unwrap_or_else(|e| error!("{}", e));
        self.remove().await.unwrap_or_else(|e| error!("{}", e));
    }
}

impl Drop for Container {
    // same drop implementation as in async-dropper-simple crate
    // https://github.com/t3hmrman/async-dropper/blob/ec6e5bbd6c894b23538cfec80375bcaefb8e5710/crates/async-dropper-simple/src/no_default_bound.rs#L111
    fn drop(&mut self) {
        if !self.dropped {
            // Prevent the copy `this` to drop again
            self.dropped = true;
            let mut this = std::mem::take(self);
            // Prevent the original `self` to drop again
            self.dropped = true;
            TokioScope::scope_and_block(|s| {
                s.spawn(async move {
                    this.async_drop().await;
                })
            });
        }
    }
}
