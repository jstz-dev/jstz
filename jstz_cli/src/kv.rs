use anyhow::Result;
use clap::Subcommand;

use crate::{config::Config, jstz::JstzClient};

async fn get(alias: Option<String>, key: String, cfg: &mut Config) -> Result<()> {
    let jstz_client = JstzClient::new(cfg);

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
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = JstzClient::new(cfg);

    let address = cfg.accounts.get_address_from(alias)?;

    let key_string = match key {
        Some(key) => key,
        None => "EMPTY".to_string(),
    };

    let value = jstz_client
        .get_subkey_list(&address.as_str(), &key_string)
        .await?;

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
        from: Option<String>,
    },
    /// List subkeys for a key
    List {
        /// Key
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        from: Option<String>,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Get { key, from } => get(from, key, cfg).await,
        Command::List { key, from } => list(from, key, cfg).await,
    }
}
