use std::fs::File;

use anyhow::{anyhow, Ok, Result};
use clap::Subcommand;
use log::info;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use std::env;

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
    /// 🔄 Restarts the sandbox.
    Restart,
}

pub async fn start(detach: bool, background: bool) -> Result<()> {
    let mut cfg = Config::load()?;

    daemon::main(detach, background, &mut cfg).await?;
    Ok(())
}

<<<<<<< HEAD
pub fn stop() -> Result<()> {
    daemon::stop_sandbox(false)?;
    Ok(())
=======
pub fn stop(restart: bool) -> Result<()> {
    let cfg = Config::load()?;

    match cfg.sandbox {
        Some(sandbox_cfg) => {
            if !restart {
                info!("Stopping the sandbox...");
            }
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;

            wait_for_termination(pid)?;

            Ok(())
        }
        None => {
            if !restart {
                bail_user_error!("The sandbox is not running!")
            } else {
                Ok(())
            }
        }
    }
>>>>>>> cea00e5 (feat(cli): restart option for sandbox)
}

pub async fn restart() -> Result<()> {
    stop(true)?;
    start(false).await
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
<<<<<<< HEAD
        Command::Start { detach, background } => start(detach, background).await,
        Command::Stop => stop(),
=======
        Command::Start { no_daemon } => start(no_daemon).await,
        Command::Stop => stop(false),
        Command::Restart => restart().await,
>>>>>>> cea00e5 (feat(cli): restart option for sandbox)
    }
}
