use anyhow::{anyhow, Result};
use clap::Subcommand;
use jstz_crypto::public_key_hash::PublicKeyHash;

use crate::config::Config;

mod trace;

#[derive(Subcommand)]
pub enum Command {
    /// View logs
    Trace {
        #[arg(value_name = "SMART_FUNCTIOIN")]
        address: String,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Trace { address } => {
            let address = PublicKeyHash::from_base58(&address)
                .map_err(|_| anyhow!("Invalid smart function address"))?;
            trace::exec(address, cfg).await
        }
    }
}
