use std::{
    fs::File,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{run_command, OctezSetup};

#[derive(Debug, Serialize, Deserialize)]
pub struct OctezNode {
    /// Path to the octez-node binary
    /// If None, the binary will inside PATH will be used
    pub octez_setup: Option<OctezSetup>,
    /// Path to the octez-node directory
    pub octez_node_dir: PathBuf,
}

const BINARY_NAME: &str = "octez-node";

fn default_command() -> Command {
    Command::new(BINARY_NAME)
}

impl OctezNode {
    /// Create a command based on the octez setup configuration
    fn command(&self, mounts: &[&str]) -> Command {
        self.octez_setup
            .as_ref()
            .map(|setup| setup.command(BINARY_NAME, mounts))
            .unwrap_or_else(default_command)
    }

    pub fn config_init(
        &self,
        network: &str,
        http_endpoint: &str,
        rpc_endpoint: &str,
        num_connections: u32,
        sandbox_params_path: &str,
    ) -> Result<()> {
        run_command(self.command(&[sandbox_params_path]).args([
            "config",
            "init",
            "--network",
            network,
            "--data-dir",
            self.octez_node_dir.to_str().expect("Invalid path"),
            "--net-addr",
            http_endpoint,
            "--rpc-addr",
            rpc_endpoint,
            "--allow-all-rpc",
            rpc_endpoint,
            "--connections",
            num_connections.to_string().as_str(),
        ]))
    }

    pub fn generate_identity(&self) -> Result<()> {
        run_command(self.command(&[]).args([
            "identity",
            "generate",
            "--data-dir",
            self.octez_node_dir.to_str().expect("Invalid path"),
        ]))
    }

    pub fn run(
        &self,
        log_file: &File,
        options: &[&str],
        sandbox_params_path: &str,
        sandbox_path: &str,
    ) -> Result<Child> {
        let mut command = self.command(&[sandbox_params_path, sandbox_path]);

        command
            .args([
                "run",
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
                "--singleprocess",
            ])
            .args(options)
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?));

        Ok(command.spawn()?)
    }
}
