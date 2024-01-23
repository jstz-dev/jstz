use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

mod trace;

#[derive(Subcommand)]
pub enum Command {
    /// View logs
    Trace {
        // The address or the alias of the smart function
        #[arg(value_name = "ALIAS|ADDRESS")]
        smart_function: String,
        // Optional log level to filter log stream
        #[arg(name = "level", short, long)]
        log_level_str: Option<String>,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Trace { smart_function, log_level_str } => trace::exec(smart_function, log_level_str, cfg).await,
    }
}
