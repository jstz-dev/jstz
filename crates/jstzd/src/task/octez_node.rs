use super::Task;
use anyhow::Result;
use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use std::{fs::File, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

use octez::OctezNode as InnerOctezNode;
use std::process::Child;

pub const DEFAULT_RPC_ENDPOINT: &str = "localhost:8732";
const DEFAULT_NETWORK: &str = "sandbox";
const DEFAULT_BINARY_PATH: &str = "octez-node";

#[derive(Clone)]
pub struct OctezNodeConfig {
    /// Path to the octez node binary.
    binary_path: PathBuf,
    /// Path to the directory where the node keeps data.
    data_dir: PathBuf,
    /// Name of the tezos network that the node instance runs on.
    network: String,
    /// HTTP endpoint of the node RPC interface, e.g. 'localhost:8732'
    rpc_endpoint: String,
    /// Path to the file that keeps octez node logs.
    log_file: PathBuf,
    /// Run options for octez node.
    options: Vec<String>,
}

#[derive(Default)]
pub struct OctezNodeConfigBuilder {
    binary_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    network: Option<String>,
    rpc_endpoint: Option<String>,
    log_file: Option<PathBuf>,
    options: Option<Vec<String>>,
}

impl OctezNodeConfigBuilder {
    pub fn new() -> Self {
        OctezNodeConfigBuilder::default()
    }

    /// Sets the path to the octez node binary.
    pub fn set_binary_path(&mut self, path: &str) -> &mut Self {
        self.binary_path = Some(PathBuf::from(path));
        self
    }

    /// Sets the path to the directory where the node keeps data.
    pub fn set_data_dir(&mut self, path: &str) -> &mut Self {
        self.data_dir = Some(PathBuf::from(path));
        self
    }

    /// Sets the name of the tezos network that the node instance runs on.
    pub fn set_network(&mut self, network: &str) -> &mut Self {
        self.network = Some(network.to_owned());
        self
    }

    /// Sets the HTTP(S) endpoint of the node RPC interface, e.g. 'http://localhost:8732'
    pub fn set_rpc_endpoint(&mut self, endpoint: &str) -> &mut Self {
        self.rpc_endpoint = Some(endpoint.to_owned());
        self
    }

    /// Sets the path to the file that keeps octez node logs.
    pub fn set_log_file(&mut self, path: &str) -> &mut Self {
        self.log_file = Some(PathBuf::from(path));
        self
    }

    /// Sets run options for octez node.
    pub fn set_run_options(&mut self, options: &[&str]) -> &mut Self {
        self.options = Some(
            options
                .iter()
                .map(|v| (*v).to_owned())
                .collect::<Vec<String>>(),
        );
        self
    }

    /// Builds a config set based on values collected.
    pub fn build(&mut self) -> Result<OctezNodeConfig> {
        Ok(OctezNodeConfig {
            binary_path: self
                .binary_path
                .take()
                .unwrap_or(PathBuf::from(DEFAULT_BINARY_PATH)),
            data_dir: self
                .data_dir
                .take()
                .unwrap_or(PathBuf::from(tempfile::TempDir::new().unwrap().path())),
            network: self.network.take().unwrap_or(DEFAULT_NETWORK.to_owned()),
            rpc_endpoint: self
                .rpc_endpoint
                .take()
                .unwrap_or(DEFAULT_RPC_ENDPOINT.to_owned()),
            log_file: self.log_file.take().unwrap_or(PathBuf::from(
                tempfile::NamedTempFile::new().unwrap().path(),
            )),
            options: self.options.take().unwrap_or_default(),
        })
    }
}

#[derive(Default)]
struct ChildWrapper {
    inner: Option<Child>,
}

impl ChildWrapper {
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(mut v) = self.inner.take() {
            return Ok(v.kill()?);
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

#[derive(Default, Clone)]
pub struct OctezNode {
    inner: Arc<RwLock<AsyncDropper<ChildWrapper>>>,
}

#[async_trait]
impl Task for OctezNode {
    type Config = OctezNodeConfig;

    /// Spins up the task with the given config.
    async fn spawn(config: Self::Config) -> Result<Self> {
        let node = InnerOctezNode {
            octez_node_bin: Some(config.binary_path),
            octez_node_dir: config.data_dir,
        };

        // localhost:8731 refers to the peer http endpoint. This will be removed
        // when we switch to the async node implementation where this option is removed
        node.config_init(&config.network, "localhost:8731", &config.rpc_endpoint, 0)?;
        node.generate_identity()?;
        Ok(OctezNode {
            inner: Arc::new(RwLock::new(AsyncDropper::new(ChildWrapper {
                inner: Some(
                    node.run(
                        &File::create(&config.log_file)?,
                        config
                            .options
                            .iter()
                            .map(|s| s as &str)
                            .collect::<Vec<&str>>()
                            .as_slice(),
                    )?,
                ),
            }))),
        })
    }

    /// Aborts the running task.
    async fn kill(&mut self) -> Result<()> {
        let mut inner = self.inner.write().await;
        Ok(inner.inner_mut().kill().await?)
    }

    /// Conducts a health check on the running task.
    async fn health_check(&self) -> Result<bool> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::task::octez_node::{
        DEFAULT_BINARY_PATH, DEFAULT_NETWORK, DEFAULT_RPC_ENDPOINT,
    };

    use super::OctezNodeConfigBuilder;

    #[test]
    fn config_builder() {
        let config = OctezNodeConfigBuilder::new()
            .set_binary_path("/tmp/binary")
            .set_data_dir("/tmp/something")
            .set_network("network")
            .set_rpc_endpoint("my_endpoint")
            .set_log_file("/log_file")
            .set_run_options(&["foo", "bar"])
            .build()
            .unwrap();
        assert_eq!(config.binary_path, PathBuf::from("/tmp/binary"));
        assert_eq!(config.data_dir, PathBuf::from("/tmp/something"));
        assert_eq!(config.network, "network".to_owned());
        assert_eq!(config.rpc_endpoint, "my_endpoint".to_owned());
        assert_eq!(config.log_file, PathBuf::from("/log_file"));
        assert_eq!(
            config.options,
            Vec::from(["foo".to_owned(), "bar".to_owned()])
        );
    }

    #[test]
    fn config_builder_default() {
        let config = OctezNodeConfigBuilder::new().build().unwrap();
        assert_eq!(config.binary_path, PathBuf::from(DEFAULT_BINARY_PATH));
        // Checks if the default path is a valid one that actually can exist in the file system
        std::fs::create_dir(config.data_dir).unwrap();
        assert_eq!(config.network, DEFAULT_NETWORK.to_owned());
        assert_eq!(config.rpc_endpoint, DEFAULT_RPC_ENDPOINT.to_owned());
        // Checks if the default path is a valid one that actually can exist in the file system
        std::fs::File::create(config.log_file).unwrap();
        assert_eq!(config.options, Vec::<String>::default());
    }
}
