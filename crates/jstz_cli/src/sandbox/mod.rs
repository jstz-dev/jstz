use anyhow::{anyhow, Ok, Result};
use clap::Subcommand;
use log::info;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};

mod daemon;

mod consts;

pub use consts::*;

use crate::{config::Config, error::bail_user_error};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 🎬 Starts the sandbox.
    Start {
        /// Do not daemonize the process.
        #[clap(long, short, default_value = "false")]
        no_daemon: bool,
    },
    /// 🛑 Stops the sandbox.
    Stop,
}

pub async fn start(no_daemon: bool) -> Result<()> {
    let mut cfg = Config::load()?;

    daemon::main(no_daemon, &mut cfg).await?;
    Ok(())
}

pub fn stop() -> Result<()> {
    let cfg = Config::load()?;

    match cfg.sandbox {
        Some(sandbox_cfg) => {
            info!("Stopping the sandbox...");
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;

            Ok(())
        }
        None => bail_user_error!("The sandbox is not running!"),
    }
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Start { no_daemon } => start(no_daemon).await,
        Command::Stop => stop(),
    }
}
