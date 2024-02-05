use clap::Subcommand;

use crate::{config::Config, error::Result, utils::AddressOrAlias};

async fn get(account: Option<AddressOrAlias>, key: String) -> Result<()> {
    let cfg = Config::load()?;
    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;

    let value = cfg.jstz_client()?.get_value(&address, key.as_str()).await?;

    // Print value
    match value {
        Some(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        None => println!("No value found"),
    }

    Ok(())
}

async fn list(account: Option<AddressOrAlias>, key: Option<String>) -> Result<()> {
    let cfg = Config::load()?;
    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;

    let value = cfg.jstz_client()?.get_subkey_list(&address, &key).await?;

    // Print list of values
    match value {
        Some(value) => {
            for item in value {
                println!("{}", item);
            }
        }
        None => println!("No values found"),
    }

    Ok(())
}

#[derive(Subcommand)]
pub enum Command {
    /// Get value for a key
    Get {
        /// Key
        #[arg(value_name = "KEY")]
        key: String,
        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<AddressOrAlias>,
    },
    /// List subkeys for a key
    List {
        /// Key
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<AddressOrAlias>,
    },
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Get { key, account } => get(account, key).await,
        Command::List { key, account } => list(account, key).await,
    }
}
