use std::{
    collections::{hash_map, HashMap},
    env, fs,
    path::PathBuf,
};

use derive_more::{From, TryInto};
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
use jstz_proto::context::account::Address;
use log::debug;
use octez::{OctezClient, OctezNode, OctezRollupNode};
use serde::{Deserialize, Serialize};

use crate::{
    error::{bail, user_error, Result},
    jstz::JstzClient,
    utils::AddressOrAlias,
};

pub fn jstz_home_dir() -> PathBuf {
    if let Ok(value) = env::var("JSTZ_HOME") {
        PathBuf::from(value)
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".jstz")
    }
}

// Represents a collection of accounts: users or smart functions
#[derive(Serialize, Deserialize, Debug, Clone, From, TryInto)]
pub enum Account {
    User(User),
    SmartFunction(SmartFunction),
}

impl Account {
    pub fn address(&self) -> &Address {
        match self {
            Account::User(user) => &user.address,
            Account::SmartFunction(sf) => &sf.address,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub address: Address,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SmartFunction {
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AccountConfig {
    current_alias: Option<String>,
    accounts: HashMap<String, Account>,
}

impl AccountConfig {
    pub fn current_alias(&self) -> Option<&str> {
        self.current_alias.as_deref()
    }

    pub fn set_current_alias(&mut self, alias: Option<String>) -> Result<()> {
        if let Some(alias) = alias.as_ref() {
            if !self.contains(alias.as_str()) {
                bail!(
                    "Cannot set current account to '{}', account not found.",
                    alias
                );
            }

            if let Some(Account::SmartFunction(_)) = self.accounts.get(alias) {
                bail!(
                    "Cannot set current account to '{}', it is a smart function.",
                    alias
                );
            }
        }

        self.current_alias = alias;

        Ok(())
    }

    pub fn current_user(&self) -> Option<(&str, &User)> {
        let alias = self.current_alias.as_ref()?;
        let account = self.accounts.get(alias)?;

        if let Account::User(user) = account {
            Some((alias, user))
        } else {
            // SAFETY: The invariant is enforced by the API (`set_current_alias`).
            panic!("Broken invariant. Current alias is not a user account.");
        }
    }

    pub fn contains(&self, alias: &str) -> bool {
        self.accounts.contains_key(alias)
    }

    pub fn insert<T: Into<Account>>(&mut self, alias: String, account: T) {
        self.accounts.insert(alias, account.into());
    }

    pub fn get(&self, alias: &str) -> Option<&Account> {
        self.accounts.get(alias)
    }

    pub fn remove(&mut self, alias: &str) -> Option<Account> {
        if self.current_alias.as_deref() == Some(alias) {
            self.current_alias = None;
        }

        self.accounts.remove(alias)
    }

    pub fn iter(&self) -> AccountsIter<'_> {
        AccountsIter {
            inner: self.accounts.iter(),
        }
    }
}

impl AddressOrAlias {
    pub fn resolve(&self, cfg: &Config) -> Result<Address> {
        match self {
            AddressOrAlias::Address(address) => Ok(address.clone()),
            AddressOrAlias::Alias(alias) => {
                let account = cfg
                    .accounts
                    .get(alias)
                    .ok_or_else(|| user_error!("User/smart function '{}' not found. Please provide a valid address or alias.", alias))?;

                Ok(account.address().clone())
            }
        }
    }

    pub fn resolve_or_use_current_user(
        account: Option<AddressOrAlias>,
        cfg: &Config,
    ) -> Result<Address> {
        match account {
            Some(account) => account.resolve(cfg),
            None => cfg
                .accounts
                .current_user()
                .ok_or(user_error!(
                    "You are not logged in. Please run `jstz login`."
                ))
                .map(|(_, user)| user.address.clone()),
        }
    }
}

pub struct AccountsIter<'a> {
    inner: hash_map::Iter<'a, String, Account>,
}

impl<'a> Iterator for AccountsIter<'a> {
    type Item = (&'a String, &'a Account);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    /// Path to octez installation
    pub octez_path: PathBuf,
    /// Sandbox config (None if sandbox is not running)
    pub sandbox: Option<SandboxConfig>,
    /// List of accounts
    #[serde(flatten)]
    pub accounts: AccountConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SandboxConfig {
    /// Directory of the octez client (initialized when sandbox is running)
    pub octez_client_dir: PathBuf,
    /// Directory of the octez node
    pub octez_node_dir: PathBuf,
    /// Directory of the octez rollup node
    pub octez_rollup_node_dir: PathBuf,
    /// Pid of the pid
    pub pid: u32,
}

pub const SANDBOX_OCTEZ_NODE_PORT: u16 = 18731;
pub const SANDBOX_OCTEZ_NODE_RPC_PORT: u16 = 18730;
pub const SANDBOX_JSTZ_NODE_PORT: u16 = 8933;
pub const SANDBOX_OCTEZ_SMART_ROLLUP_PORT: u16 = 8932;

impl Config {
    /// Path to the configuration file
    pub fn path() -> PathBuf {
        jstz_home_dir().join("config.json")
    }

    /// Load the configuration from the file
    pub fn load() -> Result<Self> {
        let path = Self::path();

        let config = if path.exists() {
            let json = fs::read_to_string(&path)?;
            debug!("Config file contents: {}", json);

            serde_json::from_str(&json).map_err(|_| {
                user_error!("Your configuration file is improperly configured.")
            })?
        } else {
            Config::default()
        };

        debug!("Config (on load): {:?}", config);

        Ok(config)
    }

    /// Save the configuration to the file
    pub fn save(&self) -> Result<()> {
        debug!("Config (on save): {:?}", self);

        let path = Self::path();

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;

        Ok(())
    }

    pub fn sandbox(&self) -> Result<&SandboxConfig> {
        self.sandbox.as_ref().ok_or(user_error!(
            "The sandbox is not running. Please run `jstz sandbox start`."
        ))
    }

    pub fn octez_client(&self) -> Result<OctezClient> {
        let sandbox = self.sandbox()?;

        Ok(OctezClient {
            octez_client_bin: Some(self.octez_path.join("octez-client")),
            octez_client_dir: sandbox.octez_client_dir.clone(),
            endpoint: format!("http://127.0.0.1:{}", SANDBOX_OCTEZ_NODE_RPC_PORT),
            disable_disclaimer: true,
        })
    }

    pub fn jstz_client(&self) -> Result<JstzClient> {
        // FIXME: Calling self.sandbox() here will raise an error if
        // the sandbox isn't running (the desired behaviour).
        //
        // In future, with network configs, the result will be used.
        let _ = self.sandbox()?;

        Ok(JstzClient::new(format!(
            "http://127.0.0.1:{}",
            SANDBOX_JSTZ_NODE_PORT
        )))
    }

    pub fn octez_node(&self) -> Result<OctezNode> {
        let sandbox = self.sandbox()?;

        Ok(OctezNode {
            octez_node_bin: Some(self.octez_path.join("octez-node")),
            octez_node_dir: sandbox.octez_node_dir.clone(),
        })
    }

    pub fn octez_rollup_node(&self) -> Result<OctezRollupNode> {
        let sandbox = self.sandbox()?;

        Ok(OctezRollupNode {
            octez_rollup_node_bin: Some(self.octez_path.join("octez-smart-rollup-node")),
            octez_rollup_node_dir: sandbox.octez_rollup_node_dir.clone(),
            octez_client_dir: sandbox.octez_client_dir.clone(),
            endpoint: format!("http://127.0.0.1:{}", SANDBOX_OCTEZ_NODE_RPC_PORT),
        })
    }
}
