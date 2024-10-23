use std::{path::PathBuf, process::Command};

use anyhow::{anyhow, Result};

pub mod r#async;
mod client;
mod node;
mod rollup;
mod thread;

pub use client::*;
pub use node::*;
pub use rollup::*;
pub use thread::*;

pub(crate) fn path_or_default<'a>(
    path: Option<&'a PathBuf>,
    default: &'a str,
) -> &'a str {
    path.and_then(|bin| bin.to_str()).unwrap_or(default)
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

pub fn unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
