use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

async fn get(
    alias: Option<String>,
    key: String,
    network: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = cfg.jstz_client(&network)?;

    let address = cfg.accounts.get_address_from(alias)?;

    let value = jstz_client
        .get_value(address.as_str(), key.as_str())
        .await?;

    // Print value
    match value {
        Some(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        None => println!("No value found"),
    }

    Ok(())
}

async fn list(
    alias: Option<String>,
    key: Option<String>,
    network: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = cfg.jstz_client(&network)?;

    let address = cfg.accounts.get_address_from(alias)?;

    let value = jstz_client.get_subkey_list(address.as_str(), &key).await?;

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
        account: Option<String>,
        /// Network to use, defaults to `default_network`` specified in config file.
        #[arg(short, long, default_value = None)]
        network: Option<String>,
    },
    /// List subkeys for a key
    List {
        /// Key
        #[arg(value_name = "KEY")]
        key: Option<String>,
        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<String>,
        /// Network to use, defaults to `default_network`` specified in config file.
        #[arg(short, long, default_value = None)]
        network: Option<String>,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Get {
            key,
            account,
            network,
        } => get(account, key, network, cfg).await,
        Command::List {
            key,
            account,
            network,
        } => list(account, key, network, cfg).await,
    }
}
