use std::{path::PathBuf, process::Command};

use anyhow::{anyhow, Result};

mod client;
mod node;
mod rollup;
mod thread;

pub use client::*;
pub use node::*;
pub use rollup::*;
use serde::{Deserialize, Serialize};
pub use thread::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OctezSetup {
    /// Process path to Octez installation
    Process(PathBuf),
    /// Docker image tag for Octez
    Docker(String),
}

impl OctezSetup {
    pub fn command(
        &self,
        binary: &str,
        mounts: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Command {
        match self {
            Self::Process(path) => Command::new(path.join(binary)),
            Self::Docker(image_tag) => {
                let mut command = Command::new("docker");

                command.args([
                    "run",
                    "--network=host",
                    &format!("--entrypoint=/usr/local/bin/{}", binary),
                    "-v",
                    "/var:/var",
                    "-v",
                    "/tmp:/tmp",
                ]);

                for path in mounts {
                    command.arg("-v");
                    command.arg(format!("{0}:{0}", path.as_ref()));
                }

                command.arg(format!("tezos/tezos:{}", image_tag));

                command
            }
        }
    }
}

pub(crate) fn run_command_with_output(command: &mut Command) -> Result<String> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "Command {:?} failed:\n {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn run_command(command: &mut Command) -> Result<()> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "Command {:?} failed:\n {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}
