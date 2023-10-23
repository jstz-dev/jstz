use anyhow::Result;
use clap::Subcommand;

pub mod account;
pub mod account_list;
mod account_management;

use crate::config::Config;

fn create_account(
    passphrase: Option<String>,
    alias: String,
    cfg: &mut Config,
) -> Result<()> {
    let passphrase = match passphrase {
        Some(passphrase) => passphrase,
        None => {
            let passphrase = account_management::generate_passphrase();
            println!("Generated passphrase: {}", passphrase);
            passphrase
        }
    };

    let address = account_management::create_account(passphrase, alias, cfg)?;
    println!("Account created: {}", address);
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
}

pub fn exec(command: Command, cfg: &mut Config) -> Result<()> {
    match command {
        Command::Create { alias, passphrase } => create_account(passphrase, alias, cfg),
    }
}
