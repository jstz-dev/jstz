use crate::unused_port;

use super::{bootstrap::SmartRollupPvmKind, endpoint::Endpoint};
use anyhow::Result;
use http::Uri;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tezos_crypto_rs::hash::SmartRollupHash;
use tokio::process::{Child, Command};

const DEFAULT_BINARY_PATH: &str = "octez-smart-rollup-node";

#[derive(Clone, PartialEq, Debug)]
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

pub struct OctezRollupConfigBuilder {
    /// global options:
    /// Path to the octez-smart-rollup-node binary
    /// If None, use `octez-smart-rollup-node`
    binary_path: Option<PathBuf>,
    /// Path to octez client base dir
    octez_client_base_dir: PathBuf,
    /// RPC endpoint for the octez-node
    octez_node_endpoint: Endpoint,
    /// Type of Proof-generating Virtual Machine (PVM) that interprets the kernel
    pvm_kind: Option<SmartRollupPvmKind>,
    /// Run options:
    /// Path to the smart rollup data directory
    data_dir: Option<RollupDataDir>,
    /// The rollup address
    address: SmartRollupHash,
    /// The rollup operator alias | address
    operator: String,
    /// The path to the kernel installer hex file
    boot_sector_file: PathBuf,
    /// HTTP endpoint of the rollup node RPC interface,
    /// if None, use localhost with random port
    pub rpc_endpoint: Option<Endpoint>,
}

impl OctezRollupConfigBuilder {
    pub fn new(
        octez_node_endpoint: Endpoint,
        octez_client_base_dir: PathBuf,
        address: SmartRollupHash,
        operator: String,
        boot_sector_file: PathBuf,
    ) -> Self {
        OctezRollupConfigBuilder {
            binary_path: None,
            pvm_kind: None,
            data_dir: None,
            octez_node_endpoint,
            octez_client_base_dir,
            address,
            operator,
            boot_sector_file,
            rpc_endpoint: None,
        }
    }

    pub fn set_binary_path(mut self, binary_path: &str) -> Self {
        self.binary_path = Some(PathBuf::from(binary_path));
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

    pub fn build(self) -> Result<OctezRollupConfig> {
        Ok(OctezRollupConfig {
            binary_path: self
                .binary_path
                .unwrap_or(PathBuf::from(DEFAULT_BINARY_PATH)),
            octez_client_base_dir: self.octez_client_base_dir,
            octez_node_endpoint: self.octez_node_endpoint,
            pvm_kind: self.pvm_kind.unwrap_or(SmartRollupPvmKind::Wasm),
            data_dir: self.data_dir.unwrap_or(RollupDataDir::Temp),
            address: self.address,
            operator: self.operator,
            boot_sector_file: self.boot_sector_file,
            rpc_endpoint: self.rpc_endpoint.unwrap_or_else(|| {
                let uri = Uri::from_str(&format!("127.0.0.1:{}", unused_port())).unwrap();
                Endpoint::try_from(uri).unwrap()
            }),
        })
    }
}

#[derive(Clone)]
pub struct OctezRollupConfig {
    pub binary_path: PathBuf,
    pub octez_client_base_dir: PathBuf,
    pub octez_node_endpoint: Endpoint,
    pub data_dir: RollupDataDir,
    pub address: SmartRollupHash,
    pub operator: String,
    pub boot_sector_file: PathBuf,
    pub rpc_endpoint: Endpoint,
    pub pvm_kind: SmartRollupPvmKind,
}

pub struct OctezRollup {
    binary_path: PathBuf,
    /// Path to the directory where the rollup state & kernel preimages are stored
    data_dir: PathBuf,
    octez_client_base_dir: PathBuf,
    octez_node_endpoint: Endpoint,
    rpc_endpoint: Endpoint,
}

impl OctezRollup {
    pub fn new(
        binary_path: &Path,
        data_dir: &Path,
        octez_client_base_dir: &Path,
        octez_node_endpoint: &Endpoint,
        rpc_endpoint: &Endpoint,
    ) -> Self {
        Self {
            binary_path: binary_path.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            octez_client_base_dir: octez_client_base_dir.to_path_buf(),
            octez_node_endpoint: octez_node_endpoint.clone(),
            rpc_endpoint: rpc_endpoint.clone(),
        }
    }
}

impl OctezRollup {
    fn command(&self) -> Command {
        let mut command = Command::new(self.binary_path.to_string_lossy().as_ref());
        command.args([
            "--endpoint",
            &self.octez_node_endpoint.to_string(),
            "--base-dir",
            &self.octez_client_base_dir.to_string_lossy(),
        ]);
        command
    }

    pub fn run(
        &self,
        address: &SmartRollupHash,
        operator: &str,
        boot_sector_file: Option<&Path>,
        kernel_debug_file: Option<&Path>,
    ) -> Result<Child> {
        let mut command = self.command();
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
        Ok(command.spawn()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn builds_rollup_config() {
        let rollup_config = OctezRollupConfigBuilder::new(
            Endpoint::localhost(1234),
            PathBuf::from("/base_dir"),
            SmartRollupHash::from_str("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK").unwrap(),
            "operator".to_owned(),
            PathBuf::from("/tmp/boot_sector.hex"),
        )
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
            format!("http://127.0.0.1:{}", port)
        );
    }
}
