use crate::config::Config;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    Trace {
        // The address of the smart function to trace the log from
        #[arg(value_name = "SMART_FUNCTION")]
        address: String,
        // `pretty` or `json`
        #[arg(short, long)]
        format: Option<String>,
        // filter by log level
        #[arg(short, long)]
        level: Option<String>,
        // filter by text match
        #[arg(short, long)]
        search: Option<String>,
    },
}

pub fn exec(command: Command, _cfg: &Config) -> Result<()> {
    match command {
        Command::Trace {
            address: _,
            format: _,
            level: _,
            search: _,
        } => {
            // TODO: connect with jstz_node for logging.
            Ok(())
        }
    }
}
