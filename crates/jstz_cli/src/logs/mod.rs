use clap::Subcommand;
use jstz_api::js_log::LogLevel;

use crate::{utils::AddressOrAlias, Result};

mod trace;

use trace::DEFAULT_LOG_LEVEL;

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
    },
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Trace {
            smart_function,
            log_level,
        } => trace::exec(smart_function, log_level).await,
    }
}
