use crate::unused_port;

use super::file::FileWrapper;
use super::{bootstrap::SmartRollupPvmKind, endpoint::Endpoint};
use anyhow::Result;
use http::Uri;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Stdio;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use tezos_crypto_rs::hash::SmartRollupHash;
use tokio::process::{Child, Command};

const DEFAULT_BINARY_PATH: &str = "octez-smart-rollup-node";

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum RollupDataDir {
    /// Path to the rollup data directory. This directory
    /// should contain the kernel pre image files under `wasm_2_0_0/`
    Path {
        data_dir: PathBuf,
    },
    /// Path to the directory containing the kernel pre image files
    /// This will be copied to `wasm_2_0_0/` in the temp directory that will be created
    TempWithPreImages {
        preimages_dir: PathBuf,
    },
    Temp,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct OctezRollupConfigBuilder {
    /// global options:
    /// Path to the octez-smart-rollup-node binary
    /// If None, use `octez-smart-rollup-node`
    binary_path: Option<PathBuf>,
    /// Path to octez client base dir
    octez_client_base_dir: Option<PathBuf>,
    /// RPC endpoint for the octez-node
    octez_node_endpoint: Option<Endpoint>,
    /// Type of Proof-generating Virtual Machine (PVM) that interprets the kernel
    pvm_kind: Option<SmartRollupPvmKind>,
    /// Run options:
    /// Path to the smart rollup data directory
    data_dir: Option<RollupDataDir>,
    /// The rollup address
    address: Option<SmartRollupHash>,
    /// The rollup operator alias | address
    operator: Option<String>,
    /// The path to the kernel installer hex file
    boot_sector_file: Option<PathBuf>,
    /// HTTP endpoint of the rollup node RPC interface,
    /// if None, use localhost with random port
    pub rpc_endpoint: Option<Endpoint>,
    /// The path to the kernel debug file
    #[serde(skip_deserializing)]
    kernel_debug_file: Option<FileWrapper>,
    /// Path to the log file.
    log_file: Option<PathBuf>,
    /// The history mode
    history_mode: HistoryMode,
}

// Manual implementation of Default to allow for custom defaults for mandatory fields in the future
#[allow(clippy::derivable_impls)]
impl Default for OctezRollupConfigBuilder {
    fn default() -> Self {
        Self {
            binary_path: None,
            octez_client_base_dir: None,
            octez_node_endpoint: None,
            pvm_kind: None,
            data_dir: None,
            address: None,
            operator: None,
            boot_sector_file: None,
            rpc_endpoint: None,
            kernel_debug_file: None,
            log_file: None,
            history_mode: HistoryMode::default(),
        }
    }
}

impl OctezRollupConfigBuilder {
    pub fn new(
        octez_node_endpoint: Endpoint,
        octez_client_base_dir: PathBuf,
        address: SmartRollupHash,
        operator: String,
        boot_sector_file: PathBuf,
        history_mode: Option<HistoryMode>,
    ) -> Self {
        OctezRollupConfigBuilder {
            binary_path: None,
            pvm_kind: None,
            data_dir: None,
            octez_node_endpoint: Some(octez_node_endpoint),
            octez_client_base_dir: Some(octez_client_base_dir),
            address: Some(address),
            operator: Some(operator),
            boot_sector_file: Some(boot_sector_file),
            rpc_endpoint: None,
            kernel_debug_file: None,
            log_file: None,
            history_mode: history_mode.unwrap_or_default(),
        }
    }

    pub fn set_binary_path(mut self, binary_path: &str) -> Self {
        self.binary_path = Some(PathBuf::from(binary_path));
        self
    }

    pub fn set_octez_client_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.octez_client_base_dir = Some(base_dir);
        self
    }

    pub fn set_octez_node_endpoint(mut self, endpoint: &Endpoint) -> Self {
        self.octez_node_endpoint = Some(endpoint.clone());
        self
    }

    pub fn set_address(mut self, address: SmartRollupHash) -> Self {
        self.address = Some(address);
        self
    }

    pub fn set_operator(mut self, operator: String) -> Self {
        self.operator = Some(operator);
        self
    }

    pub fn set_boot_sector_file(mut self, file: PathBuf) -> Self {
        self.boot_sector_file = Some(file);
        self
    }

    pub fn set_rpc_endpoint(mut self, rpc_endpoint: &Endpoint) -> Self {
        self.rpc_endpoint = Some(rpc_endpoint.clone());
        self
    }

    pub fn set_data_dir(mut self, data_dir: RollupDataDir) -> Self {
        self.data_dir = Some(data_dir);
        self
    }

    pub fn set_kernel_debug_file(mut self, kernel_debug_file: FileWrapper) -> Self {
        self.kernel_debug_file.replace(kernel_debug_file);
        self
    }

    pub fn set_log_file(mut self, path: &Path) -> Self {
        self.log_file = Some(path.into());
        self
    }

    pub fn set_history_mode(mut self, history_mode: HistoryMode) -> Self {
        self.history_mode = history_mode;
        self
    }

    // Getter methods to check if fields are set
    pub fn has_octez_client_base_dir(&self) -> bool {
        self.octez_client_base_dir.is_some()
    }

    pub fn has_octez_node_endpoint(&self) -> bool {
        self.octez_node_endpoint.is_some()
    }

    pub fn has_address(&self) -> bool {
        self.address.is_some()
    }

    pub fn has_operator(&self) -> bool {
        self.operator.is_some()
    }

    pub fn has_boot_sector_file(&self) -> bool {
        self.boot_sector_file.is_some()
    }

    pub fn build(self) -> Result<OctezRollupConfig> {
        Ok(OctezRollupConfig {
            binary_path: self
                .binary_path
                .unwrap_or(PathBuf::from(DEFAULT_BINARY_PATH)),
            octez_client_base_dir: self
                .octez_client_base_dir
                .ok_or_else(|| anyhow::anyhow!("octez_client_base_dir is required"))?,
            octez_node_endpoint: self
                .octez_node_endpoint
                .ok_or_else(|| anyhow::anyhow!("octez_node_endpoint is required"))?,
            pvm_kind: self.pvm_kind.unwrap_or(SmartRollupPvmKind::Wasm),
            data_dir: self.data_dir.unwrap_or(RollupDataDir::Temp),
            address: self
                .address
                .ok_or_else(|| anyhow::anyhow!("address is required"))?,
            operator: self
                .operator
                .ok_or_else(|| anyhow::anyhow!("operator is required"))?,
            boot_sector_file: self
                .boot_sector_file
                .ok_or_else(|| anyhow::anyhow!("boot_sector_file is required"))?,
            rpc_endpoint: self.rpc_endpoint.unwrap_or_else(|| {
                let port = unused_port();
                let uri = Uri::from_str(&format!("127.0.0.1:{port}")).unwrap();
                Endpoint::try_from(uri).unwrap()
            }),
            kernel_debug_file: self.kernel_debug_file.map(Arc::new),
            log_file: Arc::new(match self.log_file {
                Some(v) => FileWrapper::try_from(v)?,
                None => FileWrapper::default(),
            }),
            history_mode: self.history_mode,
        })
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct OctezRollupConfig {
    pub binary_path: PathBuf,
    pub octez_client_base_dir: PathBuf,
    pub octez_node_endpoint: Endpoint,
    // TODO: https://linear.app/tezos/issue/JSTZ-243/include-rollup-data-dir-in-config
    #[serde(skip_serializing)]
    pub data_dir: RollupDataDir,
    pub address: SmartRollupHash,
    pub operator: String,
    pub boot_sector_file: PathBuf,
    pub rpc_endpoint: Endpoint,
    pub pvm_kind: SmartRollupPvmKind,
    pub kernel_debug_file: Option<Arc<FileWrapper>>,
    pub log_file: Arc<FileWrapper>,
    pub history_mode: HistoryMode,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryMode {
    #[default]
    Full,
    Archive,
}

impl fmt::Display for HistoryMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HistoryMode::Full => write!(f, "full"),
            HistoryMode::Archive => write!(f, "archive"),
        }
    }
}

impl OctezRollupConfig {
    /// Create a new config with a different rollup address
    pub fn with_address(mut self, address: SmartRollupHash) -> Self {
        self.address = address;
        self
    }
}

pub struct OctezRollup {
    binary_path: PathBuf,
    /// Path to the directory where the rollup state & kernel preimages are stored
    data_dir: PathBuf,
    octez_client_base_dir: PathBuf,
    octez_node_endpoint: Endpoint,
    rpc_endpoint: Endpoint,
    log_file: Arc<FileWrapper>,
}

impl OctezRollup {
    pub fn new(
        binary_path: &Path,
        data_dir: &Path,
        octez_client_base_dir: &Path,
        octez_node_endpoint: &Endpoint,
        rpc_endpoint: &Endpoint,
        log_file: &Arc<FileWrapper>,
    ) -> Self {
        Self {
            binary_path: binary_path.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            octez_client_base_dir: octez_client_base_dir.to_path_buf(),
            octez_node_endpoint: octez_node_endpoint.clone(),
            rpc_endpoint: rpc_endpoint.clone(),
            log_file: log_file.clone(),
        }
    }
}

impl OctezRollup {
    fn command(&self) -> Result<Command> {
        let mut command = Command::new(self.binary_path.to_string_lossy().as_ref());
        command
            .args([
                "--endpoint",
                &self.octez_node_endpoint.to_string(),
                "--base-dir",
                &self.octez_client_base_dir.to_string_lossy(),
            ])
            .stdout(Stdio::from(self.log_file.as_file().try_clone()?))
            .stderr(Stdio::from(self.log_file.as_file().try_clone()?));
        Ok(command)
    }

    pub fn run(
        &self,
        address: &SmartRollupHash,
        operator: &str,
        history_mode: &HistoryMode,
        boot_sector_file: Option<&Path>,
        kernel_debug_file: Option<&Path>,
    ) -> Result<Child> {
        let mut command = self.command()?;
        command.args([
            "run",
            "operator",
            "for",
            &address.to_string(),
            "with",
            "operators",
            operator,
            "--data-dir",
            &self.data_dir.to_string_lossy(),
            "--rpc-addr",
            self.rpc_endpoint.host(),
            "--rpc-port",
            &self.rpc_endpoint.port().to_string(),
            "--acl-override",
            "allow-all",
            "--unsafe-disable-wasm-kernel-checks",
            "--history-mode",
            &history_mode.to_string(),
        ]);
        if let Some(boot_sector_file) = boot_sector_file {
            command.args(["--boot-sector-file", &boot_sector_file.to_string_lossy()]);
        }
        if let Some(kernel_debug_file) = kernel_debug_file {
            command.args([
                "--log-kernel-debug",
                "--log-kernel-debug-file",
                &kernel_debug_file.to_string_lossy(),
            ]);
        }

        // Print command and log file location for debugging
        println!("🔧 Starting octez-smart-rollup-node...");
        println!("   Command: {:?}", command.as_std());
        println!("   Log file: {}", self.log_file.as_ref());
        println!("   Data dir: {}", self.data_dir.to_string_lossy());
        println!(
            "   RPC endpoint: {}:{}",
            self.rpc_endpoint.host(),
            self.rpc_endpoint.port()
        );
        println!();

        Ok(command.spawn()?)
    }
}

#[cfg(test)]
mod test {
    use tempfile::NamedTempFile;

    use super::*;
    #[test]
    fn builds_rollup_config() {
        let kernel_debug_file = NamedTempFile::new().unwrap();
        let kernel_debug_file_path = kernel_debug_file.path().to_path_buf();
        let rollup_config = OctezRollupConfigBuilder::new(
            Endpoint::localhost(1234),
            PathBuf::from("/base_dir"),
            SmartRollupHash::from_str("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK").unwrap(),
            "operator".to_owned(),
            PathBuf::from("/tmp/boot_sector.hex"),
            None,
        )
        .set_kernel_debug_file(FileWrapper::TempFile(kernel_debug_file))
        .set_history_mode(HistoryMode::Archive)
        .build()
        .unwrap();
        assert_eq!(rollup_config.pvm_kind, SmartRollupPvmKind::Wasm);
        assert_eq!(
            rollup_config.binary_path,
            PathBuf::from(DEFAULT_BINARY_PATH)
        );
        assert_eq!(rollup_config.octez_node_endpoint, Endpoint::localhost(1234));

        assert_eq!(rollup_config.data_dir, RollupDataDir::Temp);
        assert_eq!(
            rollup_config.octez_client_base_dir,
            PathBuf::from("/base_dir")
        );
        assert_eq!(
            rollup_config.address,
            SmartRollupHash::from_str("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK").unwrap()
        );
        assert_eq!(rollup_config.operator, "operator");
        assert_eq!(
            rollup_config.boot_sector_file,
            PathBuf::from("/tmp/boot_sector.hex"),
        );
        let port = rollup_config.rpc_endpoint.port();
        assert_eq!(
            rollup_config.rpc_endpoint.to_string(),
            format!("http://127.0.0.1:{port}")
        );
        assert_eq!(
            rollup_config.kernel_debug_file.map(|v| v.path()),
            Some(kernel_debug_file_path)
        );
        assert_eq!(rollup_config.history_mode, HistoryMode::Archive);
    }

    #[test]
    fn serialize_config() {
        let kernel_debug_file = NamedTempFile::new().unwrap();
        let kernel_debug_file_path = kernel_debug_file.path().to_path_buf();
        let log_file = tempfile::NamedTempFile::new().unwrap().into_temp_path();
        let config = OctezRollupConfigBuilder::new(
            Endpoint::localhost(1234),
            PathBuf::from("/base_dir"),
            SmartRollupHash::from_str("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK").unwrap(),
            "operator".to_owned(),
            PathBuf::from("/tmp/boot_sector.hex"),
            None,
        )
        .set_kernel_debug_file(FileWrapper::TempFile(kernel_debug_file))
        .set_data_dir(RollupDataDir::TempWithPreImages {
            preimages_dir: PathBuf::from("/tmp/pre_images"),
        })
        .set_log_file(log_file.to_path_buf().as_path())
        .build()
        .unwrap();

        let json = serde_json::to_value(config.clone()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "binary_path": "octez-smart-rollup-node",
                "octez_client_base_dir": "/base_dir",
                "octez_node_endpoint": "http://localhost:1234",
                "pvm_kind": "wasm_2_0_0",
                "address": "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
                "operator": "operator",
                "boot_sector_file": "/tmp/boot_sector.hex",
                "rpc_endpoint": format!("http://127.0.0.1:{}", config.rpc_endpoint.port()),
                "kernel_debug_file": kernel_debug_file_path.to_string_lossy(),
                "log_file": log_file.to_string_lossy(),
                "history_mode": "full"
            })
        );
    }

    #[test]
    fn deserialize_minimal_config() {
        // Test that we can deserialize a config with just rpc_endpoint
        let json = serde_json::json!({
            "rpc_endpoint": "http://0.0.0.0:18741"
        });

        let builder: OctezRollupConfigBuilder = serde_json::from_value(json).unwrap();
        assert_eq!(
            builder.rpc_endpoint.unwrap(),
            Endpoint::try_from(Uri::from_str("http://0.0.0.0:18741").unwrap()).unwrap()
        );
        assert!(builder.octez_client_base_dir.is_none());
        assert!(builder.octez_node_endpoint.is_none());
        assert!(builder.address.is_none());
        assert!(builder.operator.is_none());
        assert!(builder.boot_sector_file.is_none());
    }

    #[test]
    fn deserialize_empty_config() {
        // Test that we can deserialize an empty config
        let json = serde_json::json!({});

        let builder: OctezRollupConfigBuilder = serde_json::from_value(json).unwrap();
        assert!(builder.rpc_endpoint.is_none());
        assert!(builder.octez_client_base_dir.is_none());
        assert!(builder.octez_node_endpoint.is_none());
        assert!(builder.address.is_none());
        assert!(builder.operator.is_none());
        assert!(builder.boot_sector_file.is_none());
    }

    #[test]
    fn build_fails_without_required_fields() {
        // Test that build() fails when required fields are missing
        let builder = OctezRollupConfigBuilder::default().set_rpc_endpoint(
            &Endpoint::try_from(Uri::from_str("http://0.0.0.0:18741").unwrap()).unwrap(),
        );

        let result = builder.build();
        assert!(result.is_err());
        let error_msg = format!("{}", result.unwrap_err());
        assert!(error_msg.contains("octez_client_base_dir is required"));
    }
}
