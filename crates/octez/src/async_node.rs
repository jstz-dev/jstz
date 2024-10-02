use std::{fs::File, path::PathBuf, process::Stdio};

use tokio::process::{Child, Command};

use super::path_or_default;
use anyhow::Result;

pub struct AsyncOctezNode {
    /// Path to the octez-node binary
    /// If None, the binary will inside PATH will be used
    pub octez_node_bin: Option<PathBuf>,
    /// Path to the octez-node directory
    pub octez_node_dir: PathBuf,
}

impl AsyncOctezNode {
    fn command(&self) -> Command {
        Command::new(path_or_default(self.octez_node_bin.as_ref(), "octez-node"))
    }

    pub async fn config_init(
        &self,
        network: &str,
        rpc_endpoint: &str,
        num_connections: u32,
    ) -> Result<Child> {
        Ok(self
            .command()
            .args([
                "config",
                "init",
                "--network",
                network,
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
                "--rpc-addr",
                rpc_endpoint,
                "--connections",
                num_connections.to_string().as_str(),
            ])
            .spawn()?)
    }

    pub async fn generate_identity(&self) -> Result<Child> {
        Ok(self
            .command()
            .args([
                "identity",
                "generate",
                "0",
                "--data-dir",
                self.octez_node_dir.to_str().expect("Invalid path"),
            ])
            .spawn()?)
    }

    pub async fn run(&self, log_file: &File, options: &[&str]) -> Result<Child> {
        let mut command = self.command();

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
