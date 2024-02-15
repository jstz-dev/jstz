use std::{thread, time::Duration};

use anyhow::{Ok, Result};
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
        #[clap(long, short, default_value = "false", hide = true)]
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

fn wait_for_termination(pid: Pid) -> Result<()> {
    loop {
        let result: nix::Result<()> = kill(pid, Signal::SIGTERM);
        match result {
            // Sending 0 as the signal just checks for the process existence
            core::result::Result::Ok(_) => {
                // Process exists, continue waiting
                thread::sleep(Duration::from_millis(100));
            }
            Err(nix::Error::ESRCH) => {
                // No such process, it has terminated
                break;
            }
            Err(e) => {
                // An unexpected error occurred
                bail_user_error!("Failed to kill the sandbox process: {:?}", e)
            }
        }
    }
    Ok(())
}

pub fn stop() -> Result<()> {
    let cfg = Config::load()?;

    match cfg.sandbox {
        Some(sandbox_cfg) => {
            info!("Stopping the sandbox...");
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;

            wait_for_termination(pid)?;

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
