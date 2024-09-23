use anyhow::Result;
use async_dropper::AsyncDrop;
use async_scoped::TokioScope;
use async_trait::async_trait;
use bollard::{
    container::AttachContainerOptions, secret::ContainerStateStatusEnum, Docker,
};
use futures_util::{AsyncBufRead, TryStreamExt};
use log::error;
use std::{io, sync::Arc};

#[derive(Default)]
pub struct Container {
    pub id: String,
    client: Option<Arc<Docker>>,
    dropped: bool,
    _private: (),
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

    // Creates buffered reader for stdout IO stream
    pub async fn stdout(&self) -> Result<impl AsyncBufRead> {
        let options = Some(AttachContainerOptions::<String> {
            stdout: Some(true),
            logs: Some(true),
            stream: Some(true),
            ..AttachContainerOptions::default()
        });

        self.attach_container(options).await
    }

    // Creates buffered reader for stderr IO stream
    pub async fn stderr(&self) -> Result<impl AsyncBufRead> {
        let options: Option<AttachContainerOptions<String>> =
            Some(AttachContainerOptions::<String> {
                stderr: Some(true),
                logs: Some(true),
                stream: Some(true),
                ..AttachContainerOptions::default()
            });
        self.attach_container(options).await
    }

    async fn attach_container(
        &self,
        options: Option<AttachContainerOptions<String>>,
    ) -> Result<impl AsyncBufRead> {
        let stream = self
            .client()?
            .attach_container(&self.id, options)
            .await?
            .output
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));
        match self.container_status().await? {
            Some(ContainerStateStatusEnum::RUNNING) => Ok(stream.into_async_read()),
            _ => Err(anyhow::anyhow!("Container is not running")),
        }
    }

    async fn container_status(&self) -> Result<Option<ContainerStateStatusEnum>> {
        let response = self.client()?.inspect_container(&self.id, None).await?;
        if let Some(state) = response.state {
            return Ok(state.status);
        }
        Ok(None)
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
    fn drop(&mut self) {
        if !self.dropped {
            self.dropped = true;
            let mut this = std::mem::take(self);
            self.dropped = true;
            TokioScope::scope_and_block(|s| {
                s.spawn(async move {
                    this.async_drop().await;
                })
            });
        }
    }
}
