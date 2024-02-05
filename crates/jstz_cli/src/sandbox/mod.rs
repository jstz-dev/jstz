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

pub fn start(cfg: &mut Config) -> Result<()> {
    // let sandboxd_log = File::create(cfg.jstz_path.join("/logs/sandboxd.log"))?;

    // let daemonize = Daemonize::new().stdout(sandboxd_log);

    // daemonize.start()?;

    daemon::main(cfg)
}

pub fn stop(cfg: &mut Config) -> Result<()> {
    match cfg.sandbox.take() {
        Some(sandbox_cfg) => {
            let pid = Pid::from_raw(sandbox_cfg.pid as i32);
            kill(pid, Signal::SIGTERM)?;
            Ok(())
        }
        None => bail_user_error!("The sandbox is not running!"),
    }
}

pub fn exec(cfg: &mut Config, command: Command) -> Result<()> {
    match command {
        Command::Start => start(cfg),
        Command::Stop => stop(cfg),
    }
}
