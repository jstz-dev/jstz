use anyhow::Result;
use bollard::Docker;
use log::error;
use std::sync::Arc;

pub struct Container {
    pub id: String,
    client: Option<Arc<Docker>>,
    _private: (),
}

impl Container {
    /// Creates a new container with running `id`
    pub fn new(client: Arc<Docker>, id: String) -> Self {
        Self {
            id,
            client: Some(client),
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

    pub async fn cleanup(&self) -> Result<()> {
        self.stop()
            .await
            .unwrap_or_else(|e| error!("Error stopping container: {}", e));
        match self.client()?.remove_container(&self.id, None).await {
            Ok(_) => {}
            Err(e) => error!("Error removing container {}: {}", self.id, e),
        }
        Ok(())
    }

    fn client(&self) -> Result<&Arc<Docker>> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Client does not exist"))
    }
}
