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
    /// Docker container name or ID for Octez
    Docker(String),
}

pub(crate) fn run_command_with_output(command: &mut Command) -> Result<String> {
    let output = command.output()?;

    println!("Output: {}", String::from_utf8_lossy(&output.stdout));

    if !output.status.success() {
        println!("Fail: {:?}", output.status.success());
        return Err(anyhow!(
            "Command {:?} failed:\n {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    println!("Success: {:?}", output.status.success());

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn run_command(command: &mut Command) -> Result<()> {
    println!("Running command: {:?}", command);
    let output = command.output();

    println!("Output: {:?}", output);

    let o2 = output?;

    if !o2.status.success() {
        return Err(anyhow!(
            "Command {:?} failed:\n {}",
            command,
            String::from_utf8_lossy(&o2.stderr)
        ));
    }

    Ok(())
}
