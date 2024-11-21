use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_with::SerializeDisplay;
use std::{fmt::Display, path::PathBuf};
use tokio::process::{Child, Command};

use super::{endpoint::Endpoint, protocol::Protocol};

#[derive(PartialEq, Debug, Clone, SerializeDisplay)]
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

#[derive(Clone, Serialize)]
pub struct OctezBakerConfig {
    binary_path: BakerBinaryPath,
    octez_client_base_dir: PathBuf,
    octez_node_endpoint: Endpoint,
}

#[derive(Default)]
pub struct OctezBakerConfigBuilder {
    binary_path: Option<BakerBinaryPath>,
    octez_client_base_dir: Option<PathBuf>,
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

    pub fn set_octez_node_endpoint(mut self, endpoint: &Endpoint) -> Self {
        self.octez_node_endpoint = Some(endpoint.clone());
        self
    }

    pub fn build(self) -> Result<OctezBakerConfig> {
        Ok(OctezBakerConfig {
            binary_path: self.binary_path.ok_or(anyhow!("binary path not set"))?,
            octez_client_base_dir: self
                .octez_client_base_dir
                .ok_or(anyhow!("octez_client_base_dir not set"))?,
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
    use std::str::FromStr;

    use super::*;
    use crate::r#async::endpoint::Endpoint;
    use http::Uri;
    use tempfile::TempDir;

    #[test]
    fn test_octez_baker_config_builder() {
        let base_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let config: OctezBakerConfig = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Env(Protocol::Alpha))
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build()
            .unwrap();
        assert_eq!(config.binary_path, BakerBinaryPath::Env(Protocol::Alpha));
        assert_eq!(config.octez_client_base_dir, base_dir.path());
        assert_eq!(config.octez_node_endpoint, endpoint);
    }

    #[test]
    fn octez_baker_config_builder_fails_without_binary_path() {
        let base_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let config: Result<OctezBakerConfig> = OctezBakerConfigBuilder::new()
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build();
        assert!(config.is_err_and(|e| e.to_string().contains("binary path not set")));
    }

    #[test]
    fn serialize_baker_path() {
        assert_eq!(
            serde_json::to_string(&BakerBinaryPath::Env(Protocol::Alpha)).unwrap(),
            "\"octez-baker-alpha\""
        );

        assert_eq!(
            serde_json::to_string(&BakerBinaryPath::Env(Protocol::ParisC)).unwrap(),
            "\"octez-baker-PsParisC\""
        );

        assert_eq!(
            serde_json::to_string(&BakerBinaryPath::Custom(
                PathBuf::from_str("/foo/bar").unwrap()
            ))
            .unwrap(),
            "\"/foo/bar\""
        );
    }

    #[test]
    fn serialize_config() {
        let base_dir = TempDir::new().unwrap();
        let endpoint =
            Endpoint::try_from(Uri::from_static("http://localhost:8732")).unwrap();
        let config = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Env(Protocol::Alpha))
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .build()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::json!({
                "octez_client_base_dir": base_dir.path().to_string_lossy(),
                "octez_node_endpoint": "http://localhost:8732",
                "binary_path": "octez-baker-alpha"
            })
        )
    }
}
