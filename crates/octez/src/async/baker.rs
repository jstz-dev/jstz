use anyhow::{anyhow, Result};
use std::{fmt::Display, path::PathBuf};
use tokio::process::{Child, Command};

use super::{
    client::OctezClient, endpoint::Endpoint, node_config::OctezNodeConfig,
    protocol::Protocol,
};

#[derive(PartialEq, Debug, Clone)]
pub enum BakerBinaryPath {
    Env(Protocol),   // The binary exists in $PATH
    Custom(PathBuf), // The binary is at the given path
}

impl Display for BakerBinaryPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakerBinaryPath::Env(Protocol::Alpha) => write!(f, "octez-baker-alpha"),
            BakerBinaryPath::Env(Protocol::ParisC) => {
                write!(f, "octez-baker-PsParisC")
            }
            BakerBinaryPath::Env(Protocol::Quebec) => {
                write!(f, "octez-baker-PsQuebec")
            }
            BakerBinaryPath::Custom(path) => write!(f, "{}", path.to_string_lossy()),
        }
    }
}

#[allow(dead_code)]
pub struct OctezBakerConfig {
    binary_path: BakerBinaryPath,
    octez_client_base_dir: PathBuf,
    octez_node_data_dir: PathBuf,
    octez_node_endpoint: Endpoint,
}

#[derive(Default)]
pub struct OctezBakerConfigBuilder {
    binary_path: Option<BakerBinaryPath>,
    octez_client_base_dir: Option<PathBuf>,
    octez_node_data_dir: Option<PathBuf>,
    octez_node_endpoint: Option<Endpoint>,
}

impl OctezBakerConfigBuilder {
    pub fn new() -> Self {
        OctezBakerConfigBuilder::default()
    }

    pub fn set_binary_path(mut self, binary_path: BakerBinaryPath) -> Self {
        self.binary_path = Some(binary_path);
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
        self.octez_node_endpoint = Some(endpoint.clone());
        self.octez_client_base_dir = Some(PathBuf::try_from(client.base_dir()).unwrap());
        self
    }

    pub fn build(self) -> Result<OctezBakerConfig> {
        Ok(OctezBakerConfig {
            binary_path: self.binary_path.ok_or(anyhow!("binary path not set"))?,
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
pub struct OctezBaker;

impl OctezBaker {
    pub async fn run(config: OctezBakerConfig) -> Result<Child> {
        let mut command = Command::new(config.binary_path.to_string());
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
        Ok(command.spawn()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::r#async::{
        client::OctezClientBuilder, endpoint::Endpoint,
        node_config::OctezNodeConfigBuilder,
    };
    use http::Uri;
    use tempfile::TempDir;

    #[test]
    fn test_octez_baker_config_builder() {
        let base_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let config: OctezBakerConfig = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Env(Protocol::Alpha))
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_data_dir(data_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build()
            .unwrap();
        assert_eq!(config.binary_path, BakerBinaryPath::Env(Protocol::Alpha));
        assert_eq!(config.octez_client_base_dir, base_dir.path());
        assert_eq!(config.octez_node_data_dir, data_dir.path());
        assert_eq!(config.octez_node_endpoint, endpoint);
    }

    #[test]
    fn octez_baker_config_builder_fails_without_binary_path() {
        let base_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let config: Result<OctezBakerConfig> = OctezBakerConfigBuilder::new()
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_data_dir(data_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build();
        assert!(config.is_err_and(|e| e.to_string().contains("binary path not set")));
    }

    #[tokio::test]
    async fn test_with_node_config_and_client() {
        let node_endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let temp_dir = TempDir::new().unwrap();
        let data_dir: &std::path::Path = temp_dir.path();
        let node_config = OctezNodeConfigBuilder::new()
            .set_binary_path("octez-node")
            .set_network("sandbox")
            .set_rpc_endpoint(&node_endpoint)
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
            .set_binary_path(BakerBinaryPath::Env(Protocol::Alpha))
            .with_node_and_client(&node_config, &octez_client)
            .build()
            .unwrap();
        assert_eq!(config.binary_path, BakerBinaryPath::Env(Protocol::Alpha));
        assert_eq!(config.octez_client_base_dir, base_dir);
        assert_eq!(config.octez_node_data_dir, data_dir);
        assert_eq!(config.octez_node_endpoint, node_endpoint);
    }
}
