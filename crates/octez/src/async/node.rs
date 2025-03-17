use std::{path::PathBuf, process::Stdio, sync::Arc};

use tokio::process::{Child, Command};

use crate::path_or_default;

use anyhow::Result;

use super::{endpoint::Endpoint, file::FileWrapper, node_config::OctezNodeRunOptions};

pub struct OctezNode {
    /// Path to the octez-node binary
    /// If None, the binary will inside PATH will be used
    pub octez_node_bin: Option<PathBuf>,
    /// Path to the octez-node directory
    pub octez_node_dir: PathBuf,
    /// Path to the log file.
    pub log_file: Arc<FileWrapper>,
}

impl OctezNode {
    fn command(&self) -> Result<Command> {
        let mut cmd =
            Command::new(path_or_default(self.octez_node_bin.as_ref(), "octez-node"));
        cmd.stdout(Stdio::from(self.log_file.as_file().try_clone()?))
            .stderr(Stdio::from(self.log_file.as_file().try_clone()?));
        Ok(cmd)
    }

    pub async fn config_init(
        &self,
        network: &str,
        rpc_endpoint: &Endpoint,
        p2p_address: &Endpoint,
        num_connections: u32,
    ) -> Result<Child> {
        Ok(self
            .command()?
            .args([
                "config",
                "init",
                "--network",
                network,
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
                // the node executable does not accept the scheme part
                "--rpc-addr",
                &rpc_endpoint.to_authority(),
                "--allow-all-rpc",
                &rpc_endpoint.to_authority(),
                "--connections",
                num_connections.to_string().as_str(),
                "--net-addr",
                &p2p_address.to_authority(),
            ])
            .spawn()?)
    }

    pub async fn generate_identity(&self) -> Result<Child> {
        Ok(self
            .command()?
            .args([
                "identity",
                "generate",
                "0",
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
            ])
            .spawn()?)
    }

    pub fn run(&self, options: &OctezNodeRunOptions) -> Result<Child> {
        let mut command = self.command()?;

        command
            .args([
                "run",
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
                "--singleprocess",
            ])
            .args(options.to_string().split(' '));

        Ok(command.spawn()?)
    }
}
