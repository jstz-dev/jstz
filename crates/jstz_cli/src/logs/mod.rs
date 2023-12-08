use anyhow::{anyhow, Result};
use clap::Subcommand;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::context::account::Address;

use crate::config::Config;

mod trace;

#[derive(Subcommand)]
pub enum Command {
    /// View logs
    Trace {
        // The address or the alias of the smart function
        #[arg(value_name = "ALIAS|ADDRESS")]
        smart_function: String,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Trace { smart_function } => {
            let address = get_address_for_smart_function(&smart_function, cfg)?;
            trace::exec(address, cfg).await
        }
    }
}

fn get_address_for_smart_function(smart_function: &str, cfg: &Config) -> Result<Address> {
    if let Ok(address) = PublicKeyHash::from_base58(smart_function) {
        return Ok(address);
    }

    if let Ok(account) = cfg.accounts.get(smart_function) {
        return Ok(account.address().clone());
    }

    Err(anyhow!("Invalid smart function alias or address"))
}
