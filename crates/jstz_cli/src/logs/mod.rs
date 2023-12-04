use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

mod trace;

#[derive(Subcommand)]
pub enum Command {
    /// View logs
    Trace {
        /// View console.log messages
        #[arg(long)]
        log: bool,
        /// View console.info messages
        #[arg(long)]
        info: bool,
        /// View console.warn messages
        #[arg(long)]
        warn: bool,
        /// View console.error messages
        #[arg(long)]
        error: bool,
        /// View contract creations
        #[arg(long)]
        contract: bool,
        /// Add custom search strings
        #[arg(long, value_name = "custom_strings")]
        custom: Vec<String>,
    },
}

pub fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Trace {
            log,
            info,
            warn,
            error,
            contract,
            custom,
        } => trace::exec(log, info, warn, error, contract, custom, cfg),
    }
}
