use anyhow::{Ok, Result};
use clap::Subcommand;

pub mod daemon;

mod consts;

pub use consts::*;

use crate::config::Config;

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

pub async fn start(detach: bool, background: bool) -> Result<()> {
    let mut cfg = Config::load_sync()?;

    daemon::main(detach, background, &mut cfg).await?;
    Ok(())
}

pub fn stop() -> Result<()> {
    daemon::stop_sandbox(false)?;
    Ok(())
}

pub async fn restart(detach: bool) -> Result<()> {
    daemon::stop_sandbox(true)?;
    start(detach, false).await
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Start { detach, background } => start(detach, background).await,
        Command::Stop => stop(),
        Command::Restart { detach } => restart(detach).await,
    }
}
