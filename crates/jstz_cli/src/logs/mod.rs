use clap::Subcommand;
use jstz_core::log_record::LogLevel;

use crate::{config::NetworkName, utils::AddressOrAlias, Result};

mod trace;

pub use trace::exec_trace;
pub use trace::DEFAULT_LOG_LEVEL;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 📜 Starts a log tracing session for a deployed smart function.
    Trace {
        // The address or the alias of the deployed smart function.
        #[arg(value_name = "ALIAS|ADDRESS")]
        smart_function: AddressOrAlias,
        // Optional log level to filter log stream.
        #[arg(name = "level", short, long, ignore_case = true, default_value_t = DEFAULT_LOG_LEVEL)]
        log_level: LogLevel,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Trace {
            smart_function,
            log_level,
            network,
        } => trace::exec(smart_function, log_level, &network).await,
    }
}
