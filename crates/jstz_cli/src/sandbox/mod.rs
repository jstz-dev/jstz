use anyhow::{Ok, Result};
use clap::Subcommand;

mod daemon;

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
}

pub async fn start(detach: bool, background: bool) -> Result<()> {
    let mut cfg = Config::load()?;

    daemon::main(detach, background, &mut cfg).await?;
    Ok(())
}

pub fn stop() -> Result<()> {
    daemon::stop_sandbox(false)?;
    Ok(())
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Start { detach, background } => start(detach, background).await,
        Command::Stop => stop(),
    }
}
