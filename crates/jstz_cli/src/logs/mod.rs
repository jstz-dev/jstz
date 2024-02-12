use clap::Subcommand;
use jstz_api::js_log::LogLevel;

use crate::{config::NetworkName, utils::AddressOrAlias, Result};

mod trace;

use trace::DEFAULT_LOG_LEVEL;

mod persist;

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
    /// View the persistent logs in the DB
    Persist {
        /// The address or the alias of the target smart function
        #[arg(value_name = "ALIAS|ADDRESS")]
        smart_function: AddressOrAlias,
        /// Reeuest id of the target smart function
        #[arg(default_value = None)]
        request_id: Option<String>,
        /// Pagination limit, defaults to `100`.
        /// Ignored if [REQUEST_ID] is provided.
        #[arg(short, long, default_value = None)]
        limit: Option<usize>,
        /// Pagination offset, defaults to `0`
        /// Ignored if [REQUEST_ID] is provided.
        #[arg(short, long, default_value = None)]
        offset: Option<usize>,
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
        Command::Persist {
            smart_function,
            request_id,
            limit,
            offset,
            network,
        } => persist::exec(smart_function, request_id, limit, offset, &network).await,
    }
}
