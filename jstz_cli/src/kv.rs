use anyhow::Result;
use clap::Subcommand;

use crate::{config::Config, jstz::JstzClient};

async fn get(alias: Option<String>, key: String, cfg: &mut Config) -> Result<()> {
    println!("get key: {}", key);
    println!("alias: {:?}", alias);

    let jstz_client = JstzClient::new(cfg);

    let account = cfg.accounts.account_or_current_mut(alias)?;

    println!("account: {:?}", account);

    let value = jstz_client
        .get_value(account.address().clone().to_base58().as_str(), key.as_str())
        .await?;

    println!("{:?}", value);

    Ok(())
}

async fn list(
    alias: Option<String>,
    key: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = JstzClient::new(cfg);

    let account = cfg.accounts.account_or_current_mut(alias)?;

    let key_string = match key {
        Some(key) => key,
        None => "".to_string(),
    };

    let value = jstz_client
        .get_subkey_list(account.address().clone().to_base58().as_str(), &key_string)
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
        #[arg(value_name = "ALIAS")]
        alias: Option<String>,
    },
    /// List subkeys for a key
    List {
        /// Key
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// User address or alias
        #[arg(short, long)]
        account: Option<String>,
    },
}

pub async fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Get { key, alias } => get(alias, key, cfg).await,
        Command::List { key, account } => list(account, key, cfg).await,
    }
}
