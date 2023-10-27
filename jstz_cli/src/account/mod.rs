use anyhow::{anyhow, Result};
use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;

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

fn login(alias: String, cfg: &mut Config) -> Result<()> {
    let account = cfg.accounts().get(&alias)?;

    println!(
        "Logged in to account {} with address {}",
        account.alias, account.address
    );

    cfg.accounts().current_alias = Some(alias);
    cfg.save()?;

    Ok(())
}

fn whoami(cfg: &Config) -> Result<()> {
    let alias = cfg
        .accounts
        .current_alias
        .as_ref()
        .ok_or(anyhow!("Not logged in!"))?;

    let account = cfg.accounts.get(&alias)?;

    println!(
        "Logged in to account {} with address {}",
        account.alias, account.address
    );

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
    /// Logs in to an account
    Login {
        /// User alias
        #[arg(value_name = "ALIAS")]
        alias: String,
    },
    /// Shows the current account
    WhoAmI {},
}

pub fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Create { alias, passphrase } => create_account(passphrase, alias, cfg),
        Command::Login { alias } => login(alias, cfg),
        Command::WhoAmI {} => whoami(cfg),
    }
}
