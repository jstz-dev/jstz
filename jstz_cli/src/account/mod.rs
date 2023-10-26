use anyhow::Result;
use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;
use std::io;
use std::io::Write;

pub mod account;

use crate::config::Config;

fn generate_passphrase() -> String {
    let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
    return mnemonic.to_string();
}

fn create_account(
    passphrase: Option<String>,
    alias: String,
    cfg: &mut Config,
) -> Result<()> {
    let passphrase = match passphrase {
        Some(passphrase) => passphrase,
        None => {
            let passphrase = generate_passphrase();
            println!("Generated passphrase: {}", passphrase);
            passphrase
        }
    };

    let account = account::Account::from_passphrase(passphrase, alias)?;

    println!("Account created with address: {}", account.address);

    cfg.accounts().upsert(account);
    cfg.save()?;

    Ok(())
}

fn delete_account(alias: String, cfg: &mut Config) -> Result<()> {
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

fn login(alias: String, cfg: &mut Config) -> Result<()> {
    let account = cfg.accounts().get(&alias);

    if account.is_none() {
        println!("Account {} does not exist", alias);
        return Ok(());
    }

    println!(
        "Logged in to account {} with address {}",
        account.unwrap().alias,
        account.unwrap().address
    );

    cfg.accounts().current_alias = Some(alias);
    cfg.save()?;

    Ok(())
}

fn logout(cfg: &mut Config) -> Result<()> {
    cfg.accounts().current_alias = None;
    cfg.save()?;
    Ok(())
}

fn whoami(cfg: &mut Config) -> Result<()> {
    let alias = cfg.accounts().current_alias.clone();

    if alias.is_none() {
        println!("Not logged in");
        return Ok(());
    }

    let account = cfg.accounts().get(&alias.unwrap()).unwrap();

    println!(
        "Logged in to account {} with address {}",
        account.alias, account.address
    );

    Ok(())
}

fn list(cfg: &mut Config) -> Result<()> {
    let accounts = cfg.accounts().list_all();

    println!("Accounts:");
    for (alias, account) in accounts {
        println!("{}: {}", alias, account.address);
    }

    Ok(())
}

#[derive(Subcommand)]
pub enum Command {
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
    /// Logs in to an account
    Login {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// Logs out of the current account
    Logout {},
    /// Shows the current account
    WhoAmI {},
    /// Lists all accounts
    List {},
}

pub fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Create { alias, passphrase } => create_account(passphrase, alias, cfg),
        Command::Delete { alias } => delete_account(alias, cfg),
        Command::Login { alias } => login(alias, cfg),
        Command::Logout {} => logout(cfg),
        Command::WhoAmI {} => whoami(cfg),
        Command::List {} => list(cfg),
    }
}
