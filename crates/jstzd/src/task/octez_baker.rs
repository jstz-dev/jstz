use super::{
    child_wrapper::{ChildWrapper, SharedChildWrapper},
    endpoint::Endpoint,
    octez_client::OctezClient,
    octez_node::OctezNodeConfig,
    Task,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use http::Uri;

use std::{path::PathBuf, str::FromStr};
use tokio::process::Command;

#[derive(PartialEq, Debug, Clone)]
pub enum Protocol {
    Alpha,
    ParisC,
    Quebec,
    Custom(PathBuf),
}

impl ToString for Protocol {
    fn to_string(&self) -> String {
        match self {
            Protocol::Alpha => "octez-baker-alpha".to_string(),
            Protocol::ParisC => "octez-baker-PsParisC".to_string(),
            Protocol::Quebec => "octez-baker-PsQuebec".to_string(),
            Protocol::Custom(path) => path.to_string_lossy().into_owned(),
        }
    }
}

#[allow(dead_code)]
pub struct OctezBakerConfig {
    protocol: Protocol,
    octez_client_base_dir: PathBuf,
    octez_node_data_dir: PathBuf,
    octez_node_endpoint: Endpoint,
}

#[derive(Default)]
pub struct OctezBakerConfigBuilder {
    protocol: Option<Protocol>,
    octez_client_base_dir: Option<PathBuf>,
    octez_node_data_dir: Option<PathBuf>,
    octez_node_endpoint: Option<Endpoint>,
}

impl OctezBakerConfigBuilder {
    pub fn new() -> Self {
        OctezBakerConfigBuilder::default()
    }

    pub fn set_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = Some(protocol);
        self
    }

    pub fn set_octez_client_base_dir(mut self, base_dir: &str) -> Self {
        self.octez_client_base_dir = Some(PathBuf::from(base_dir));
        self
    }

    pub fn set_octez_node_data_dir(mut self, data_dir: &str) -> Self {
        self.octez_node_data_dir = Some(PathBuf::from(data_dir));
        self
    }

    pub fn set_octez_node_endpoint(mut self, endpoint: &Endpoint) -> Self {
        self.octez_node_endpoint = Some(endpoint.clone());
        self
    }

    pub fn with_node_and_client(
        mut self,
        node_config: &OctezNodeConfig,
        client: &OctezClient,
    ) -> Self {
        self.octez_node_data_dir = Some(node_config.data_dir.clone());
        let endpoint = &node_config.rpc_endpoint;
        let uri = Uri::from_str(endpoint).unwrap();
        self.octez_node_endpoint = Some(uri.try_into().unwrap());
        self.octez_client_base_dir = Some(PathBuf::try_from(&client.base_dir).unwrap());
        self
    }

    pub fn build(self) -> Result<OctezBakerConfig> {
        Ok(OctezBakerConfig {
            protocol: self.protocol.ok_or(anyhow!("protocol not set"))?,
            octez_client_base_dir: self
                .octez_client_base_dir
                .ok_or(anyhow!("octez_client_base_dir not set"))?,
            octez_node_data_dir: self
                .octez_node_data_dir
                .clone()
                .ok_or(anyhow!("octez_node_data_dir not set"))?,
            octez_node_endpoint: self
                .octez_node_endpoint
                .ok_or(anyhow!("octez_node_endpoint not set"))?,
        })
    }
}

#[allow(dead_code)]
pub struct OctezBaker {
    inner: SharedChildWrapper,
    config: OctezBakerConfig,
}

#[async_trait]
impl Task for OctezBaker {
    type Config = OctezBakerConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let mut command = Command::new(config.protocol.to_string());
        command.args([
            "--base-dir",
            &config.octez_client_base_dir.to_string_lossy(),
            "--endpoint",
            &config.octez_node_endpoint.to_string(),
            "run",
            "remotely",
            "--liquidity-baking-toggle-vote",
            "pass",
        ]);
        let child = command.spawn()?;
        let inner = ChildWrapper::new_shared(child);
        Ok(OctezBaker { inner, config })
    }

    /// Aborts the running task.
    async fn kill(&mut self) -> Result<()> {
        let mut lock = self.inner.write().await;
        lock.kill().await
    }

    /// Conducts a health check on the running task.
    async fn health_check(&self) -> Result<bool> {
        // TODO: https://linear.app/tezos/issue/JSTZ-182/octez-baker-health-check
        todo!()
    }
}

#[cfg(test)]
mod test {
    use tempfile::TempDir;

    use crate::task::{octez_client::OctezClientBuilder, octez_node};

    use super::*;
    #[test]
    fn test_octez_baker_config_builder() {
        let base_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_str("http://localhost:8732").unwrap()).unwrap();
        let config: OctezBakerConfig = OctezBakerConfigBuilder::new()
            .set_protocol(Protocol::Alpha)
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_data_dir(data_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build()
            .unwrap();
        assert_eq!(config.protocol, Protocol::Alpha);
        assert_eq!(config.octez_client_base_dir, base_dir.path());
        assert_eq!(config.octez_node_data_dir, data_dir.path());
        assert_eq!(config.octez_node_endpoint, endpoint);
    }

    #[test]
    fn octez_baker_config_builder_fails_without_protocol() {
        let base_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_str("http://localhost:8732").unwrap()).unwrap();
        let config: Result<OctezBakerConfig> = OctezBakerConfigBuilder::new()
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_data_dir(data_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build();
        assert!(config.is_err_and(|e| e.to_string().contains("protocol not set")));
    }

    #[tokio::test]
    async fn test_with_node_config_and_client() {
        let node_endpoint =
            Endpoint::try_from(Uri::from_str("http://localhost:8732").unwrap()).unwrap();
        let temp_dir = TempDir::new().unwrap();
        let data_dir: &std::path::Path = temp_dir.path();
        let node_config = octez_node::OctezNodeConfigBuilder::new()
            .set_binary_path("octez-node")
            .set_network("sandbox")
            .set_rpc_endpoint(&node_endpoint.to_string())
            .set_data_dir(data_dir.to_str().unwrap())
            .build()
            .expect("Failed to build node config");

        let temp_dir = TempDir::new().unwrap();
        let base_dir: std::path::PathBuf = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_endpoint(node_endpoint.clone())
            .set_base_dir(base_dir.clone())
            .build()
            .expect("Failed to build octez client");
        let config: OctezBakerConfig = OctezBakerConfigBuilder::new()
            .set_protocol(Protocol::Alpha)
            .with_node_and_client(&node_config, &octez_client)
            .build()
            .unwrap();
        assert_eq!(config.protocol, Protocol::Alpha);
        assert_eq!(config.octez_client_base_dir, base_dir);
        assert_eq!(config.octez_node_data_dir, data_dir);
        assert_eq!(config.octez_node_endpoint, node_endpoint);
    }
}
