use anyhow::Result;
use bollard::Docker;
use std::sync::Arc;

pub struct Container {
    pub id: String,
    client: Option<Arc<Docker>>,
    _private: (),
}

impl Container {
    /// Creates a new container with running `id`
    pub(super) fn new(client: Arc<Docker>, id: String) -> Self {
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

    // Stop and remove the container, should be called when dropping the container
    pub async fn cleanup(&self) -> Result<()> {
        self.stop().await?;
        self.remove().await
    }

    fn client(&self) -> Result<&Arc<Docker>> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Client does not exist"))
    }
}
