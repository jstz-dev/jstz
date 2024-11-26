use std::{
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::task::child_wrapper::ChildWrapper;

use super::{child_wrapper::SharedChildWrapper, Task};
use anyhow::Result;
use async_trait::async_trait;
use octez::r#async::{
    directory::Directory,
    endpoint::Endpoint,
    rollup::{OctezRollupConfig, RollupDataDir},
};
use serde::Deserialize;

#[derive(Clone)]
pub struct OctezRollup {
    inner: SharedChildWrapper,
    config: OctezRollupConfig,
    // holds the TempDir instance so that the directory does not get deleted too soon
    _data_dir: Arc<Directory>,
}

impl OctezRollup {
    pub fn rpc_endpoint(&self) -> &Endpoint {
        &self.config.rpc_endpoint
    }
}

#[derive(Debug, Deserialize)]
struct HealthCheckResponse {
    healthy: bool,
}

#[async_trait]
impl Task for OctezRollup {
    type Config = OctezRollupConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let data_dir = match &config.data_dir {
            RollupDataDir::Path { data_dir } => Directory::Path(data_dir.clone()),
            RollupDataDir::TempWithPreImages {
                preimages_dir: from,
            } => {
                let temp_dir = Directory::default();
                let to = Path::new(&temp_dir).join(config.pvm_kind.to_string());
                symlink(from, to)?;
                temp_dir
            }
            RollupDataDir::Temp => Directory::default(),
        };
        let rollup = octez::r#async::rollup::OctezRollup::new(
            &config.binary_path,
            &PathBuf::from(&data_dir),
            &config.octez_client_base_dir,
            &config.octez_node_endpoint,
            &config.rpc_endpoint,
        );
        let inner = ChildWrapper::new_shared(rollup.run(
            &config.address,
            &config.operator,
            Some(&config.boot_sector_file),
            config.kernel_debug_file.as_deref(),
        )?);
        Ok(Self {
            inner,
            config,
            _data_dir: Arc::new(data_dir),
        })
    }

    async fn kill(&mut self) -> Result<()> {
        let mut inner = self.inner.write().await;
        Ok(inner.inner_mut().kill().await?)
    }

    async fn health_check(&self) -> Result<bool> {
        let res =
            reqwest::get(format!("{}/health/", &self.config.rpc_endpoint.to_string()))
                .await?;
        let body = res.json::<HealthCheckResponse>().await?;
        return Ok(body.healthy);
    }
}
