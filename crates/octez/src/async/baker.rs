use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr,
    sync::Arc,
};
use tokio::process::{Child, Command};

use super::{endpoint::Endpoint, file::FileWrapper, protocol::Protocol};

#[derive(PartialEq, Debug, Clone, SerializeDisplay, DeserializeFromStr)]
pub enum BakerBinaryPath {
    Env(Protocol),   // The binary exists in $PATH
    Custom(PathBuf), // The binary is at the given path
}

impl FromStr for BakerBinaryPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            #[cfg(not(feature = "disable-alpha"))]
            "octez-baker-alpha" => BakerBinaryPath::Env(Protocol::Alpha),
            "octez-baker-PsRiotum" => BakerBinaryPath::Env(Protocol::Rio),
            "octez-baker-PsQuebec" => BakerBinaryPath::Env(Protocol::Quebec),
            _ => BakerBinaryPath::Custom(PathBuf::from_str(s)?),
        })
    }
}

impl Display for BakerBinaryPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(feature = "disable-alpha"))]
            BakerBinaryPath::Env(Protocol::Alpha) => write!(f, "octez-baker-alpha"),
            BakerBinaryPath::Env(Protocol::Rio) => {
                write!(f, "octez-baker-PsRiotum")
            }
            BakerBinaryPath::Env(Protocol::Quebec) => {
                write!(f, "octez-baker-PsQuebec")
            }
            BakerBinaryPath::Custom(path) => write!(f, "{}", path.to_string_lossy()),
        }
    }
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct OctezBakerConfig {
    binary_path: BakerBinaryPath,
    octez_client_base_dir: PathBuf,
    octez_node_endpoint: Endpoint,
    log_file: Arc<FileWrapper>,
}

#[derive(Default, Deserialize, Debug, PartialEq)]
pub struct OctezBakerConfigBuilder {
    binary_path: Option<BakerBinaryPath>,
    octez_client_base_dir: Option<PathBuf>,
    octez_node_endpoint: Option<Endpoint>,
    /// Path to the log file.
    log_file: Option<PathBuf>,
}

impl OctezBakerConfigBuilder {
    pub fn new() -> Self {
        OctezBakerConfigBuilder::default()
    }

    pub fn set_binary_path(mut self, binary_path: BakerBinaryPath) -> Self {
        self.binary_path = Some(binary_path);
        self
    }

    pub fn binary_path(&self) -> &Option<BakerBinaryPath> {
        &self.binary_path
    }

    pub fn set_octez_client_base_dir(mut self, base_dir: &str) -> Self {
        self.octez_client_base_dir = Some(PathBuf::from(base_dir));
        self
    }

    pub fn octez_client_base_dir(&self) -> &Option<PathBuf> {
        &self.octez_client_base_dir
    }

    pub fn set_octez_node_endpoint(mut self, endpoint: &Endpoint) -> Self {
        self.octez_node_endpoint = Some(endpoint.clone());
        self
    }

    pub fn octez_node_endpoint(&self) -> &Option<Endpoint> {
        &self.octez_node_endpoint
    }

    pub fn set_log_file(mut self, path: &Path) -> Self {
        self.log_file.replace(path.into());
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
            log_file: Arc::new(match self.log_file {
                Some(v) => FileWrapper::try_from(v)?,
                None => FileWrapper::default(),
            }),
        })
    }
}

#[allow(dead_code)]
pub struct OctezBaker;

impl OctezBaker {
    pub async fn run(config: OctezBakerConfig) -> Result<Child> {
        let mut command = Command::new(config.binary_path.to_string());
        command
            .args([
                "--base-dir",
                &config.octez_client_base_dir.to_string_lossy(),
                "--endpoint",
                &config.octez_node_endpoint.to_string(),
                "run",
                "remotely",
                "--liquidity-baking-toggle-vote",
                "pass",
                "--without-dal",
            ])
            .stdout(Stdio::from(config.log_file.as_file().try_clone()?))
            .stderr(Stdio::from(config.log_file.as_file().try_clone()?));
        Ok(command.spawn()?)
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;
    use crate::r#async::endpoint::Endpoint;
    use http::Uri;
    use tempfile::{NamedTempFile, TempDir};

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
        #[cfg(not(feature = "disable-alpha"))]
        assert_eq!(
            serde_json::to_string(&BakerBinaryPath::Env(Protocol::Alpha)).unwrap(),
            "\"octez-baker-alpha\""
        );

        assert_eq!(
            serde_json::to_string(&BakerBinaryPath::Env(Protocol::Rio)).unwrap(),
            "\"octez-baker-PsRiotum\""
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
        let log_file = NamedTempFile::new().unwrap().into_temp_path();
        let config = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Env(Protocol::Rio))
            .set_octez_client_base_dir(base_dir.path().to_str().unwrap())
            .set_octez_node_endpoint(&endpoint)
            .set_log_file(log_file.to_path_buf().as_path())
            .build()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::json!({
                "octez_client_base_dir": base_dir.path().to_string_lossy(),
                "octez_node_endpoint": "http://localhost:8732",
                "binary_path": "octez-baker-PsRiotum",
                "log_file": log_file.to_string_lossy()
            })
        )
    }

    #[test]
    fn baker_path_from_str() {
        #[cfg(not(feature = "disable-alpha"))]
        assert_eq!(
            BakerBinaryPath::from_str("octez-baker-alpha").unwrap(),
            BakerBinaryPath::Env(Protocol::Alpha)
        );
        assert_eq!(
            BakerBinaryPath::from_str("octez-baker-PsRiotum").unwrap(),
            BakerBinaryPath::Env(Protocol::Rio)
        );
        assert_eq!(
            BakerBinaryPath::from_str("/foo/bar").unwrap(),
            BakerBinaryPath::Custom(PathBuf::from_str("/foo/bar").unwrap())
        );
    }

    #[test]
    fn deserialize_baker_path() {
        #[cfg(not(feature = "disable-alpha"))]
        assert_eq!(
            serde_json::from_str::<BakerBinaryPath>("\"octez-baker-alpha\"").unwrap(),
            BakerBinaryPath::Env(Protocol::Alpha)
        );
        assert_eq!(
            serde_json::from_str::<BakerBinaryPath>("\"octez-baker-PsRiotum\"").unwrap(),
            BakerBinaryPath::Env(Protocol::Rio)
        );
        assert_eq!(
            serde_json::from_str::<BakerBinaryPath>("\"/foo/bar\"").unwrap(),
            BakerBinaryPath::Custom(PathBuf::from_str("/foo/bar").unwrap())
        );
    }
}
