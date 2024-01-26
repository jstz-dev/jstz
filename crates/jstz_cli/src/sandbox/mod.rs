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

/// Endpoint details for the `octez-smart-rollup-node`
pub const DEFAULT_ROLLUP_NODE_RPC_ADDR: &str = "127.0.0.1";
pub const DEFAULT_ROLLUP_RPC_PORT: u16 = 8932;

/// Endpoint defailts for the `jstz-node`
pub const ENDPOINT: (&str, u16) = ("127.0.0.1", 8933);

pub const DEFAULT_KERNEL_FILE_PATH: &str = "logs/kernel.log";

#[derive(Subcommand)]
pub enum Command {
    /// Starts a sandbox.
    Start,
    /// Stops the sandbox.
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
