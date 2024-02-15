use std::{
    fs::File,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{path_or_default, run_command};

#[derive(Debug, Serialize, Deserialize)]
pub struct OctezNode {
    /// Path to the octez-node binary
    /// If None, the binary will inside PATH will be used
    pub octez_node_bin: Option<PathBuf>,
    /// Path to the octez-node directory
    pub octez_node_dir: PathBuf,
}

impl OctezNode {
    fn command(&self) -> Command {
        Command::new(path_or_default(self.octez_node_bin.as_ref(), "octez-node"))
    }

    pub fn config_init(
        &self,
        network: &str,
        http_endpoint: &str,
        rpc_endpoint: &str,
        num_connections: u32,
    ) -> Result<()> {
        run_command(self.command().args([
            "config",
            "init",
            "--network",
            network,
            "--data-dir",
            self.octez_node_dir.to_str().expect("Invalid path"),
            "--net-addr",
            http_endpoint,
            "--local-rpc-addr", // TODO: @alanmarkoTrilitech --local-rpc-addr is not present in future versions of octez-node
            rpc_endpoint,
            "--connections",
            num_connections.to_string().as_str(),
        ]))
    }

    pub fn generate_identity(&self) -> Result<()> {
        run_command(self.command().args([
            "identity",
            "generate",
            "--data-dir",
            self.octez_node_dir.to_str().expect("Invalid path"),
        ]))
    }

    pub fn run(&self, log_file: &File, options: &[&str]) -> Result<Child> {
        Ok(self
            .command()
            .args([
                "run",
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
                "--singleprocess",
            ])
            .args(options)
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?))
            .spawn()?)
    }
}
