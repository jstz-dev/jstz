use anyhow::{anyhow, Ok, Result};
use clap::Subcommand;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};

mod daemon;

use crate::{
    config::Config,
    error::{bail_user_error, Result},
};

#[derive(Subcommand)]
pub enum Command {
    /// Starts a sandbox.
    Start,
    /// Stops the sandbox.
    Stop,
}

pub async fn start() -> Result<()> {
    let mut cfg = Config::load()?;

    daemon::main(cfg).await?;
    Ok(())
}

pub fn stop() -> Result<()> {
    let cfg = Config::load()?;

    match cfg.sandbox {
        Some(sandbox_cfg) => {
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;
            Ok(())
        }
        None => bail_user_error!("The sandbox is not running!"),
    }
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Start => start().await,
        Command::Stop => stop(),
    }
}
