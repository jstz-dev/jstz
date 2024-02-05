use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;
use jstz_crypto::keypair_from_passphrase;
use jstz_proto::context::account::Address;
use std::io;
use std::io::Write;

use crate::{
    config::{Account, Config, Deployment, User},
    error::{bail_user_error, user_error, Result},
    utils::AddressOrAlias,
};

fn generate_passphrase() -> String {
    let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
    mnemonic.to_string()
}

impl User {
    pub fn from_passphrase(passphrase: String) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str())?;

        let address = Address::try_from(&pk)?;

        Ok(Self {
            address,
            secret_key: sk,
            public_key: pk,
        })
    }
}

fn add_deployment(alias: String, address: Address) -> Result<()> {
    let mut cfg = Config::load()?;

    if cfg.accounts.contains(&alias) {
        bail_user_error!(
            "The deployment '{}' already exists. Please choose another name.",
            alias
        );
    }

    cfg.accounts.insert(alias, Deployment { address });
    cfg.save()?;

    Ok(())
}

fn create_account(alias: String, passphrase: Option<String>) -> Result<()> {
    let mut cfg = Config::load()?;

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
            println!("Generated passphrase: {}", passphrase);
            passphrase
        }
    };

    let user = User::from_passphrase(passphrase)?;
    println!("Account created with address: {}", user.address);

    cfg.accounts.insert(alias, user);
    cfg.save()?;

    Ok(())
}

fn delete_account(alias: String) -> Result<()> {
    let mut cfg = Config::load()?;

    if !cfg.accounts.contains(&alias) {
        bail_user_error!("The account '{}' does not exist.", alias);
    }

    // Determine the confirmation message based on the login status. Use the name in the message.
    let confirmation_message = if cfg.accounts.current_alias() == Some(&alias) {
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
        bail_user_error!("Account deletion aborted.");
    }

    cfg.accounts.remove(&alias);
    cfg.save()?;

    println!("Account '{}' successfully deleted.", alias);
    Ok(())
}

pub fn login(alias: String) -> Result<()> {
    let mut cfg = Config::load()?;

    let account = cfg.accounts.get(&alias).ok_or(user_error!(
        "Account '{}' not found. Please provide a alias or use `jstz account create`.",
        alias
    ))?;

    if cfg.accounts.current_alias() == Some(&alias) {
        bail_user_error!(
            "You are already logged in to '{}'. Please logout first using `jstz logout`.",
            alias
        )
    }

    match account {
        Account::Deployment(_) => bail_user_error!(
            "Cannot set current account to '{}', it is a deployment.",
            alias
        ),
        Account::User(user) => {
            println!(
                "Logged in to account {} with address {}",
                alias, user.address
            );

            cfg.accounts.set_current_alias(Some(alias))?;
            cfg.save()?;

            Ok(())
        }
    }
}

pub fn logout() -> Result<()> {
    let mut cfg = Config::load()?;

    if cfg.accounts.current_alias().is_none() {
        bail_user_error!("You are not logged in. Please run `jstz login`.");
    }

    cfg.accounts.set_current_alias(None)?;
    cfg.save()?;

    println!("You have been logged out.");

    Ok(())
}

pub fn whoami() -> Result<()> {
    let cfg = Config::load()?;

    let (alias, user) = cfg.accounts.current_user().ok_or(user_error!(
        "You are not logged in. Please run `jstz login`."
    ))?;

    println!(
        "Logged in to account {} with address {}",
        alias, user.address
    );

    Ok(())
}

fn list_accounts(long: bool) -> Result<()> {
    let cfg = Config::load()?;

    println!("Accounts:");
    for (alias, account) in cfg.accounts.iter() {
        if long {
            println!("Alias: {}", alias);
            match account {
                Account::User(User {
                    address,
                    secret_key,
                    public_key,
                }) => {
                    println!("  Type: User");
                    println!("  Address: {}", address);
                    println!("  Public Key: {}", public_key.to_string());
                    println!("  Secret Key: {}", secret_key.to_string());
                }
                Account::Deployment(Deployment { address, .. }) => {
                    println!("  Type: Deployment");
                    println!("  Address: {}", address);
                }
            }
        } else {
            println!("{}: {}", alias, account.address());
        }
    }

    Ok(())
}

async fn get_code(account: Option<AddressOrAlias>) -> Result<()> {
    let cfg = Config::load()?;

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    let code = cfg.jstz_client()?.get_code(&address).await?;

    match code {
        Some(code) => println!("{}", code),
        None => eprintln!("No code found"),
    }

    Ok(())
}

async fn get_balance(account: Option<AddressOrAlias>) -> Result<()> {
    let cfg = Config::load()?;

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    let balance = cfg.jstz_client()?.get_balance(&address).await?;

    println!("Balance of {} is {} $CTEZ", address, balance);

    Ok(())
}

#[derive(Subcommand)]
pub enum Command {
    /// Creates alias
    Alias {
        /// Alias
        #[arg(value_name = "ALIAS")]
        alias: String,
        /// Address
        #[arg(value_name = "ADDRESS")]
        address: Address,
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
        account: Option<AddressOrAlias>,
    },
    /// Get account balance
    Balance {
        /// User address or alias
        #[arg(short, long, value_name = "ALIAS|ADDRESS")]
        account: Option<AddressOrAlias>,
    },
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Alias { alias, address } => add_deployment(alias, address),
        Command::Create { alias, passphrase } => create_account(alias, passphrase),
        Command::Delete { alias } => delete_account(alias),
        Command::List { long } => list_accounts(long),
        Command::Code { account } => get_code(account).await,
        Command::Balance { account } => get_balance(account).await,
    }
}
