use bip39::{Language, Mnemonic, MnemonicType};
use clap::Subcommand;
use jstz_crypto::keypair_from_passphrase;
use jstz_proto::context::account::Address;
use log::{debug, info};
use std::io;
use std::io::Write;

use crate::{
    config::{Account, Config, SmartFunction, User},
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

fn add_smart_function(alias: String, address: Address) -> Result<()> {
    let mut cfg = Config::load()?;

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

    info!("{}", confirmation_message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    debug!("User input: {:?}", input);

    if !input.trim().eq(&alias) {
        bail_user_error!("Account deletion aborted.");
    }

    cfg.accounts.remove(&alias);
    cfg.save()?;

    info!("Account '{}' successfully deleted.", alias);
    Ok(())
}

pub fn login(alias: String) -> Result<()> {
    let mut cfg = Config::load()?;

    let account = cfg.accounts.get(&alias).ok_or(user_error!(
        "Account '{}' not found. Please provide a alias or use `jstz account create`.",
        alias
    ))?;

    debug!("Account found: {:?}", account);

    if cfg.accounts.current_alias() == Some(&alias) {
        bail_user_error!(
            "You are already logged in to '{}'. Please logout first using `jstz logout`.",
            alias
        )
    }

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

pub fn logout() -> Result<()> {
    let mut cfg = Config::load()?;

    if cfg.accounts.current_alias().is_none() {
        bail_user_error!("You are not logged in. Please run `jstz login`.");
    }

    cfg.accounts.set_current_alias(None)?;
    cfg.save()?;

    info!("You have been logged out.");

    Ok(())
}

pub fn whoami() -> Result<()> {
    let cfg = Config::load()?;

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

fn list_accounts(long: bool) -> Result<()> {
    let cfg = Config::load()?;

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
                    println!("  Type: Smart Function");
                    println!("  Address: {}", address);
                }
            }
        } else {
            info!("{}: {}", alias, account.address());
        }
    }

    Ok(())
}

async fn get_code(account: Option<AddressOrAlias>) -> Result<()> {
    let cfg = Config::load()?;

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    debug!("resolved `account` -> {:?}", address);

    let code = cfg
        .jstz_client()?
        .get_code(&address)
        .await?
        .ok_or(user_error!("No code found for account {}", address))?;

    info!("{}", code);

    Ok(())
}

async fn get_balance(account: Option<AddressOrAlias>) -> Result<()> {
    let cfg = Config::load()?;

    let address = AddressOrAlias::resolve_or_use_current_user(account, &cfg)?;
    debug!("resolved `account` -> {:?}", address);

    let balance = cfg.jstz_client()?.get_balance(&address).await?;

    info!("Balance of {} is {} $CTEZ", address, balance);

    Ok(())
}

#[derive(Debug, Subcommand)]
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
        Command::Alias { alias, address } => add_smart_function(alias, address),
        Command::Create { alias, passphrase } => create_account(alias, passphrase),
        Command::Delete { alias } => delete_account(alias),
        Command::List { long } => list_accounts(long),
        Command::Code { account } => get_code(account).await,
        Command::Balance { account } => get_balance(account).await,
    }
}
