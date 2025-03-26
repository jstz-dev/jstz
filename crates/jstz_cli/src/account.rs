use std::collections::hash_map::Entry;

use bip39::{Language, Mnemonic};
use clap::Subcommand;
use dialoguer::{Confirm, Input};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_crypto::{keypair_from_passphrase, public_key_hash::PublicKeyHash};
use log::{debug, info, warn};

use crate::utils::MUTEZ_PER_TEZ;
use crate::{
    config::{Account, Config, NetworkName, SmartFunction, User},
    error::{bail_user_error, user_error, Result},
    utils::AddressOrAlias,
};

fn generate_passphrase() -> String {
    // unwrap is okay here because we are using a fixed value for word count and it's always
    // valid unless the library stops supporting word count 12
    let mnemonic = Mnemonic::generate_in(Language::English, 12)
        .expect("generate_in should generate mnemonics");
    mnemonic.to_string()
}

impl User {
    pub fn from_passphrase(passphrase: String) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str())?;

        let address = PublicKeyHash::from(&pk);

        Ok(Self {
            address,
            secret_key: sk,
            public_key: pk,
        })
    }
}

async fn add_smart_function(alias: String, address: SmartFunctionHash) -> Result<()> {
    let mut cfg = Config::load().await?;
    if cfg.accounts.contains(&alias) {
        bail_user_error!(
            "The smart function '{}' already exists. Please choose another name.",
            alias
        );
    }

    info!("Added smart function: {} -> {}", alias, address);
    cfg.accounts.insert(alias, SmartFunction { address });

    cfg.save()?;

    Ok(())
}

async fn create_account(alias: String, passphrase: Option<String>) -> Result<()> {
    let mut cfg = Config::load().await?;

    if cfg.accounts.contains(&alias) {
        bail_user_error!(
            "The account '{}' already exists. Please choose another name.",
            alias
        );
    }

    let passphrase = match passphrase {
        Some(passphrase) => passphrase,
        None => {
            let passphrase = generate_passphrase();
            info!("Generated passphrase: {}", passphrase);
            passphrase
        }
    };

    let user = User::from_passphrase(passphrase)?;

    debug!("User created: {:?}", user);
    info!("User created with address: {}", user.address);

    cfg.accounts.insert(alias, user);
    cfg.save()?;

    Ok(())
}

async fn delete_account(alias: String) -> Result<()> {
    let mut cfg = Config::load().await?;

    if !cfg.accounts.contains(&alias) {
        bail_user_error!("The account '{}' does not exist.", alias);
    }

    if cfg.accounts.current_alias() == Some(&alias) {
        warn!("You are currently logged into the account: {}.", alias);
    }

    let confirmation_alias: String = Input::new().with_prompt(format!("Are you sure you want to delete the account {0}? Please type '{0}' to confirm", alias)).interact()?;

    debug!("User input: {:?}", confirmation_alias);

    if confirmation_alias != alias {
        bail_user_error!("Account deletion aborted.");
    }

    cfg.accounts.remove(&alias);
    cfg.save()?;

    info!("Account '{}' successfully deleted.", alias);
    Ok(())
}

pub async fn login(alias: String) -> Result<()> {
    let mut cfg = Config::load().await?;

    if cfg.accounts.current_alias().is_some()
        && !Confirm::new()
            .with_prompt(format!(
                "You are already logged in. Do you want to logout and login in to {}?",
                alias
            ))
            .default(true)
            .interact()?
    {
        bail_user_error!("Login aborted");
    }

    let account: &Account = match cfg.accounts.entry(alias.clone()) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            if !Confirm::new()
                .with_prompt("Account not found. Do you want to create it?")
                .interact()?
            {
                bail_user_error!("Login aborted");
            }

            let passphrase: String = Input::new()
                .with_prompt("Enter the passphrase for the new account (or leave empty to generate a random one)")
                .allow_empty(true)
                .interact()?;

            let passphrase = if passphrase.is_empty() {
                let generated_passphrase = generate_passphrase();
                info!("Generated passphrase: {}", generated_passphrase);
                generated_passphrase
            } else {
                passphrase
            };

            let user = User::from_passphrase(passphrase)?;

            entry.insert(user.into())
        }
    };

    debug!("Account: {:?}", account);

    match account {
        Account::SmartFunction(_) => {
            bail_user_error!("Cannot log into '{}', it is a smart function.", alias)
        }
        Account::User(user) => {
            info!(
                "Logged in to account {} with address {}",
                alias, user.address
            );

            cfg.accounts.set_current_alias(Some(alias))?;
            cfg.save()?;

            Ok(())
        }
    }
}

pub async fn login_quick(cfg: &mut Config) -> Result<()> {
    if cfg.accounts.current_user().is_none() {
        let account_alias: String = Input::new()
                .with_prompt("You are not logged in. Please type the account name that you want to log into or create as new")
                .interact()?;

        login(account_alias).await?;
        info!("");
    }
    Ok(())
}

pub async fn logout() -> Result<()> {
    let mut cfg = Config::load().await?;

    if cfg.accounts.current_alias().is_none() {
        bail_user_error!("You are not logged in. Please run `jstz login`.");
    }

    cfg.accounts.set_current_alias(None)?;
    cfg.save()?;

    info!("You have been logged out.");

    Ok(())
}

pub async fn whoami() -> Result<()> {
    let cfg = Config::load().await?;

    let (alias, user) = cfg.accounts.current_user().ok_or(user_error!(
        "You are not logged in. Please run `jstz login`."
    ))?;

    debug!("Current user ({:?}): {:?}", alias, user);

    info!(
        "Logged in to account {} with address {}",
        alias, user.address
    );

    Ok(())
}

async fn list_accounts(long: bool) -> Result<()> {
    let cfg = Config::load().await?;

    info!("Accounts:");
    for (alias, account) in cfg.accounts.iter() {
        if long {
            info!("Alias: {}", alias);
            match account {
                Account::User(User {
                    address,
                    secret_key,
                    public_key,
                }) => {
                    info!("  Type: User");
                    info!("  Address: {}", address);
                    info!("  Public Key: {}", public_key.to_string());
                    info!("  Secret Key: {}", secret_key.to_string());
                }
                Account::SmartFunction(SmartFunction { address, .. }) => {
                    info!("  Type: Smart Function");
                    info!("  Address: {}", address);
                }
            }
        } else {
            info!("{}: {}", alias, account.address());
        }
    }

    Ok(())
}

async fn get_code(
    account: Option<AddressOrAlias>,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;

    debug!("Getting code.. {:?}.", network);

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    let sf_address = address
        .as_smart_function()
        .ok_or(user_error!("Address is not a smart function"))?;
    debug!("resolved `account` -> {:?}", address);
    let code = cfg
        .jstz_client(&network)?
        .get_code(sf_address)
        .await?
        .ok_or(user_error!("No code found for account {}", address))?;

    info!("{}", code);

    Ok(())
}

async fn get_balance(
    account: Option<AddressOrAlias>,
    network: Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    debug!("resolved `account` -> {:?}", address);

    let balance = cfg.jstz_client(&network)?.get_balance(&address).await?;
    let tez_balance = balance as f64 / MUTEZ_PER_TEZ as f64;

    info!("{}ꜩ", tez_balance);

    Ok(())
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 🌐 Creates a user account.
    Create {
        /// User alias.
        #[arg(value_name = "ALIAS")]
        alias: String,
        /// User passphrase. If undefined, a random passphrase will be generated.
        #[arg(short, long)]
        passphrase: Option<String>,
    },
    /// ❌ Deletes an account (user or smart function).
    Delete {
        /// User or smart function alias to be deleted.
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// 📋 Lists all accounts.
    #[clap(alias = "ls")]
    List {
        /// Flag for long format output.
        #[arg(short, long)]
        long: bool,
    },
    /// 💻 Outputs the deployed code for an account.
    Code {
        /// Smart function address or alias.
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<AddressOrAlias>,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 📈 Outputs the balance of an account.
    Balance {
        /// Address or alias of the account (user or smart function).
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<AddressOrAlias>,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 🔄 Creates alias for a deployed smart function.
    Alias {
        /// Alias of the smart function.
        #[arg(value_name = "ALIAS")]
        alias: String,
        /// Address of the smart function.
        #[arg(value_name = "KT1 ADDRESS")]
        address: SmartFunctionHash,
    },
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Alias { alias, address } => add_smart_function(alias, address).await,
        Command::Create { alias, passphrase } => create_account(alias, passphrase).await,
        Command::Delete { alias } => delete_account(alias).await,
        Command::List { long } => list_accounts(long).await,
        Command::Code { account, network } => get_code(account, network).await,
        Command::Balance { account, network } => get_balance(account, network).await,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_passphrase() {
        // Just to make sure that generate_passphrase works with the current version of Mnemonic.
        // If anything changes in the library and fails our logic, the unwrap call will lead to
        // a panic and we can capture that issue here.
        assert_ne!(super::generate_passphrase(), super::generate_passphrase());
    }
}
