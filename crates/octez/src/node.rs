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

impl OctezNode {
    /// Create a command based on the octez setup configuration
    fn command(&self, mounts: &[String]) -> Command {
        match &self.octez_setup {
            Some(OctezSetup::Process(path)) => {
                let bin_path = path.join("octez-node");
                Command::new(bin_path)
            }
            Some(OctezSetup::Docker(container_name)) => {
                let mut cmd = Command::new("docker");

                let mut args = vec![
                    "run".to_string(),
                    "--network=host".to_string(),
                    "--entrypoint=/usr/local/bin/octez-node".to_string(),
                    "-v".to_string(),
                    "/var:/var".to_string(),
                    "-v".to_string(),
                    "/tmp:/tmp".to_string(),
                ];

                // Iterate over the host paths to mount, using fixed container paths
                for path in mounts {
                    args.push("-v".to_string());
                    args.push(format!("{}:{}", path, path));
                }

                args.push(container_name.to_string());

                cmd.args(args);
                cmd
            }
            None => Command::new("octez-node"),
        }
    }

    pub fn config_init(
        &self,
        network: &str,
        http_endpoint: &str,
        rpc_endpoint: &str,
        num_connections: u32,
        sandbox_params_path: &str,
    ) -> Result<()> {
        let mut binding = self.command(&[sandbox_params_path.to_string()]);
        let c = binding.args([
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
        ]);
        println!("{:?}", c);
        run_command(c)
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
        let mut command =
            self.command(&[sandbox_params_path.to_string(), sandbox_path.to_string()]);

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

        println!("WOOHOO {:?}", command);
        println!(
            "octez_node_dir {:?}",
            self.octez_node_dir.to_str().expect("Invalid path")
        );

        let res = command.spawn();

        println!("res {:?}", res);

        Ok(res?)
    }
}
