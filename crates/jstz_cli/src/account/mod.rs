use anyhow::{anyhow, Result};
use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;
use std::io;
use std::io::Write;

pub mod account;

use crate::{
    account::account::{Account, AliasAccount, OwnedAccount},
    config::Config,
};

fn generate_passphrase() -> String {
    let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
    mnemonic.to_string()
}

fn alias(address: String, name: String, cfg: &mut Config) -> Result<()> {
    cfg.accounts().add_alias(name, address)?;
    cfg.save()?;

    Ok(())
}
fn create_account(
    passphrase: Option<String>,
    alias: String,
    cfg: &mut Config,
) -> Result<()> {
    if cfg.accounts().contains(&alias) {
        return Err(anyhow!("Account already exists"));
    }

    let passphrase = match passphrase {
        Some(passphrase) => passphrase,
        None => {
            let passphrase = generate_passphrase();
            println!("Generated passphrase: {}", passphrase);
            passphrase
        }
    };

    let account = OwnedAccount::new(passphrase, alias)?;

    println!("Account created with address: {}", account.address);

    cfg.accounts().upsert(account);
    cfg.save()?;

    Ok(())
}

fn delete_account(alias: String, cfg: &mut Config) -> Result<()> {
    if !cfg.accounts().contains(&alias) {
        return Err(anyhow!("Account not found"));
    }

    // Determine the confirmation message based on the login status. Use the name in the message.
    let confirmation_message = if cfg.accounts().current_alias.as_ref() == Some(&alias) {
        format!(
            "You are currently logged into the account {}. Are you sure you want to delete it? Please type the account name to confirm: ",
            alias
        )
    } else {
        format!(
            "Are you sure you want to delete the account {}? Please type the account name to confirm: ",
            alias
        )
    };

    print!("{}", confirmation_message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq(&alias) {
        println!("Account deletion aborted.");
        return Ok(());
    }

    cfg.accounts().remove(&alias);
    cfg.save()?;

    println!("Account successfully deleted.");
    Ok(())
}

pub fn login(alias: String, cfg: &mut Config) -> Result<()> {
    if !cfg.accounts().contains(&alias) {
        return Err(anyhow!("Account not found"));
    }
    if cfg.accounts().current_alias.as_ref() == Some(&alias) {
        return Err(anyhow!("Already logged in to account {}!", alias));
    }

    let account = cfg.accounts().get(&alias)?;

    let OwnedAccount { alias, address, .. } = account.as_owned()?;
    println!("Logged in to account {} with address {}", alias, address);

    cfg.accounts().current_alias = Some(alias.to_string());
    cfg.save()?;

    Ok(())
}

pub fn logout(cfg: &mut Config) -> Result<()> {
    if cfg.accounts().current_alias.is_none() {
        return Err(anyhow!("Not logged in!"));
    }

    cfg.accounts().current_alias = None;
    cfg.save()?;
    Ok(())
}

pub fn whoami(cfg: &Config) -> Result<()> {
    let alias = cfg
        .accounts
        .current_alias
        .as_ref()
        .ok_or(anyhow!("Not logged in!"))?;

    let OwnedAccount { alias, address, .. } = cfg.accounts.get(alias)?.as_owned()?;

    println!("Logged in to account {} with address {}", alias, address);

    Ok(())
}

fn list_accounts(long: bool, cfg: &mut Config) -> Result<()> {
    let accounts = cfg.accounts().list_all();

    println!("Accounts:");
    for (alias, account) in accounts {
        if long {
            println!("Alias: {}", alias);
            match account {
                Account::Owned(OwnedAccount {
                    alias: _,
                    address,
                    secret_key,
                    public_key,
                }) => {
                    println!("  Type: Owned");
                    println!("  Address: {}", address);
                    println!("  Public Key: {}", public_key.to_string());
                    println!("  Secret Key: {}", secret_key.to_string());
                }
                Account::Alias(AliasAccount { address, .. }) => {
                    println!("  Type: Alias");
                    println!("  Address: {}", address);
                }
            }
        } else {
            println!("{}: {}", account.alias(), account.address());
        }
    }

    Ok(())
}

async fn get_code(
    account: Option<String>,
    network: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = cfg.jstz_client(&network)?;
    let address = cfg.accounts.get_address_from(account)?;

    let code = jstz_client.get_code(address.as_str()).await?;

    match code {
        Some(code) => println!("{}", code),
        None => println!("No code found"),
    }

    Ok(())
}

async fn get_balance(
    account: Option<String>,
    network: Option<String>,
    cfg: &mut Config,
) -> Result<()> {
    let jstz_client = cfg.jstz_client(&network)?;
    let address = cfg.accounts.get_address_from(account)?;

    let balance = jstz_client.get_balance(address.as_str()).await?;

    println!("{}", balance);

    Ok(())
}

#[derive(Subcommand)]
pub enum Command {
    /// Creates alias
    Alias {
        /// Address
        #[arg(value_name = "ADDRESS")]
        address: String,
        /// Name
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Creates account
    Create {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
        /// User passphrase. If undefined, a random passphrase will be generated.
        #[arg(short, long)]
        passphrase: Option<String>,
    },
    /// Deletes account
    Delete {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// Lists all accounts
    #[clap(alias = "ls")]
    List {
        /// Option for long format output
        #[arg(short, long)]
        long: bool,
    },
    /// Get account code
    Code {
        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<String>,
        /// Network to use, defaults to `default_network`` specified in config file.
        #[arg(short, long, default_value = None)]
        network: Option<String>,
    },
    /// Get account balance
    Balance {
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
        Command::Alias { address, name } => alias(address, name, cfg),
        Command::Create { alias, passphrase } => create_account(passphrase, alias, cfg),
        Command::Delete { alias } => delete_account(alias, cfg),
        Command::List { long } => list_accounts(long, cfg),
        Command::Code { account, network } => get_code(account, network, cfg).await,
        Command::Balance { account, network } => get_balance(account, network, cfg).await,
    }
}
