use std::{
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::task::child_wrapper::ChildWrapper;

use super::{child_wrapper::SharedChildWrapper, Task};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use octez::r#async::{
    directory::Directory,
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
                let to = PathBuf::from(&temp_dir).join(config.pvm_kind.to_string());
                create_dir_all(&to)?;
                copy_files(from, &to)?;
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
            None,
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

/// Copy all files from `src_dir` to `dest_dir`
fn copy_files(src_dir: &Path, dest_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(src_dir)? {
        let path = entry?.path();
        if path.is_file() {
            let file_name = path
                .file_name()
                .ok_or_else(|| anyhow!("file name not found in path: {:?}", path))?;
            let dest_file = dest_dir.join(file_name);
            fs::copy(&path, &dest_file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::copy_files;

    #[test]
    fn test_copy_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dest_dir = temp_dir.path().join("dest");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::create_dir(&dest_dir).unwrap();
        let src_file = src_dir.join("file.txt");
        std::fs::write(&src_file, "hello").unwrap();
        copy_files(&src_dir, &dest_dir).unwrap();
        let dest_file = dest_dir.join("file.txt");
        assert!(dest_file.exists());
        assert_eq!(std::fs::read_to_string(&dest_file).unwrap(), "hello");
    }
}
