use clap::Subcommand;
use log::info;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};

mod daemon;

use crate::{
    config::Config,
    error::{bail_user_error, Result},
};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 🎬 Starts the sandbox.
    Start,
    /// 🛑 Stops the sandbox.
    Stop,
}

pub fn start() -> Result<()> {
    let mut cfg = Config::load()?;

    // let sandboxd_log = File::create(cfg.jstz_path.join("/logs/sandboxd.log"))?;

    // let daemonize = Daemonize::new().stdout(sandboxd_log);

    // daemonize.start()?;

    daemon::main(&mut cfg)
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

pub fn exec(command: Command) -> Result<()> {
    match command {
        Command::Start => start(),
        Command::Stop => stop(),
    }
}
