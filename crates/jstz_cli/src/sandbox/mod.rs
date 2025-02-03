mod consts;
mod container;
pub mod daemon;

use crate::{config::Config, utils::using_jstzd};
use anyhow::{bail, Result};
use clap::Subcommand;
pub use consts::*;
use container::*;

const SANDBOX_CONTAINER_NAME: &str = "jstz-sandbox";
const SANDBOX_IMAGE: &str = "ghcr.io/jstz-dev/jstz/jstzd:0.1.1-alpha.0";

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 🎬 Starts the sandbox.
    Start {
        /// Detach the process to run in the background.
        #[clap(long, short, default_value = "false")]
        detach: bool,
        /// Run the sandbox in the background without showing any output.
        #[clap(long, short, default_value = "false", hide = true)]
        background: bool,
    },
    /// 🛑 Stops the sandbox.
    Stop,
    /// 🔄 Restarts the sandbox.
    Restart {
        /// Detach the process to run in the background.
        #[clap(long, short, default_value = "false")]
        detach: bool,
    },
}

pub async fn start(detach: bool, background: bool, use_container: bool) -> Result<()> {
    let mut cfg = Config::load_sync()?;

    match use_container {
        true => {
            start_container(SANDBOX_CONTAINER_NAME, SANDBOX_IMAGE, detach, &mut cfg)
                .await?
        }
        _ => daemon::main(detach, background, &mut cfg).await?,
    };
    Ok(())
}

pub async fn stop(use_container: bool) -> Result<bool> {
    let mut cfg = Config::load_sync()?;
    match use_container {
        true => stop_container(SANDBOX_CONTAINER_NAME, &mut cfg).await,
        _ => {
            daemon::stop_sandbox(false)?;
            Ok(true)
        }
    }
}

pub async fn restart(detach: bool, use_container: bool) -> Result<()> {
    if !stop(use_container).await? {
        return Ok(());
    }
    start(detach, false, use_container).await
}

pub async fn exec(use_container: bool, command: Command) -> Result<()> {
    if using_jstzd() {
        bail!(
            "Jstz sandbox is not available when environment variable `USE_JSTZD` is truthy."
        );
    }
    match command {
        Command::Start { detach, background } => {
            start(detach, background, use_container).await
        }
        Command::Stop => {
            stop(use_container).await?;
            Ok(())
        }
        Command::Restart { detach } => restart(detach, use_container).await,
    }
}
