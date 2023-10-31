use anyhow::{anyhow, Result};
use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;
use std::io;
use std::io::Write;

pub mod account;

use crate::{account::account::Account, config::Config};

fn generate_passphrase() -> String {
    let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
    return mnemonic.to_string();
}

fn alias(address: String, name: String, cfg: &mut Config) -> Result<()> {
    cfg.accounts().add_alias(address, name)?;
    cfg.save()?;

    Ok(())
}
fn create_account(
    passphrase: Option<String>,
    alias: String,
    function_code: Option<String>,
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

    let account = account::Account::from_passphrase(passphrase, alias, function_code)?;

    match account {
        account::Account::Owned { ref address, .. } => {
            println!("Account created with address: {}", address);
        }
        _ => unreachable!(),
    }

    cfg.accounts().upsert(account);
    cfg.save()?;

    Ok(())
}

fn delete_account(alias: String, cfg: &mut Config) -> Result<()> {
    if !cfg.accounts().contains(&alias) {
        return Err(anyhow!("Account not found"));
    }

    // Determine the confirmation message based on the login status
    let confirmation_message = if cfg.accounts().current_alias.as_ref() == Some(&alias) {
        "You are currently logged into this account. Are you sure you want to delete it? (y/N): "
    } else {
        "Are you sure you want to delete this account? (y/N): "
    };

    print!("{}", confirmation_message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !["y", "Y", "yes", "Yes"].contains(&input.trim()) {
        println!("Account deletion aborted.");
        return Ok(());
    }

    cfg.accounts().remove(&alias);
    cfg.save()?;

    println!("Account successfully deleted.");
    Ok(())
}

pub fn login(alias: String, cfg: &mut Config) -> Result<()> {
    let (alias, address) = match cfg.accounts().get(&alias) {
        Ok(Account::Owned { alias, address, .. }) => (alias, address),
        _ => {
            return Err(anyhow!("Owned account not found"));
        }
    };

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

pub fn whoami(cfg: &mut Config) -> Result<()> {
    let alias = cfg
        .accounts
        .current_alias
        .as_ref()
        .ok_or(anyhow!("Not logged in!"))?;

    let (alias, address) = match cfg.accounts.get(&alias)? {
        Account::Owned { alias, address, .. } => (alias, address),
        _ => unreachable!(),
    };

    println!("Logged in to account {} with address {}", alias, address);

    Ok(())
}

fn list(long: bool, cfg: &mut Config) -> Result<()> {
    let accounts = cfg.accounts().list_all();

    println!("Accounts:");
    for (alias, account) in accounts {
        if long {
            println!("Alias: {}", alias);
            match account {
                Account::Owned {
                    nonce,
                    address,
                    secret_key,
                    public_key,
                    function_code,
                    ..
                } => {
                    println!("  Type: Owned");
                    println!("  Nonce: {:?}", nonce);
                    println!("  Address: {}", address);
                    println!("  Public Key: {}", public_key.to_string());
                    println!("  Secret Key: {}", secret_key.to_string());
                    if let Some(func_code) = &function_code {
                        println!("  Function Code: {}", func_code);
                    } else {
                        println!("  Function Code: None");
                    }
                }
                Account::Alias { address, .. } => {
                    println!("  Type: Alias");
                    println!("  Address: {}", address);
                }
            }
        } else {
            match account {
                Account::Owned { alias, address, .. } => {
                    println!("{}: {}", alias, address)
                }
                Account::Alias { alias, address, .. } => {
                    println!("{}: {}", alias, address)
                }
            }
        }
    }

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
        /// Function code
        #[arg(short, long)]
        function_code: Option<String>,
    },
    /// Deletes account
    Delete {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// Lists all accounts
    List {
        /// Option for long format output
        #[arg(short, long)]
        long: bool,
    },
}

pub fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Alias { address, name } => alias(address, name, cfg),
        Command::Create {
            alias,
            passphrase,
            function_code,
        } => create_account(passphrase, alias, function_code, cfg),
        Command::Delete { alias } => delete_account(alias, cfg),
        Command::List { long } => list(long, cfg),
    }
}
