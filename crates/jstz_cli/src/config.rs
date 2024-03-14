use std::{
    collections::{hash_map, HashMap},
    env, fmt, fs,
    path::PathBuf,
    str::FromStr,
};

use derive_more::{From, TryInto};
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
use jstz_proto::context::account::Address;
use log::debug;
use octez::{OctezClient, OctezNode, OctezRollupNode, OctezSetup};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::{
    error::{bail, user_error, Result},
    jstz::JstzClient,
    utils::AddressOrAlias,
};

use crate::sandbox::{
    SANDBOX_JSTZ_NODE_PORT, SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_RPC_PORT,
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

pub fn default_sandbox_logs_dir() -> PathBuf {
    jstz_home_dir().join("sandbox-logs")
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
    #[serde(skip_serializing_if = "Option::is_none")]
    current_alias: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
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

    pub fn entry(&mut self, alias: String) -> hash_map::Entry<String, Account> {
        self.accounts.entry(alias)
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
    /// Octez setup configuration: Process path or Docker container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub octez_setup: Option<OctezSetup>,
    /// Sandbox config (None if sandbox is not running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// List of accounts
    #[serde(flatten)]
    pub accounts: AccountConfig,
    /// Available networks
    #[serde(flatten)]
    pub networks: NetworkConfig,
    /// Sandbox logs dir
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_logs_dir: Option<PathBuf>,
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

#[derive(SerializeDisplay, DeserializeFromStr, Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkName {
    Custom(String),
    // Dev network uses sandbox config
    Dev,
}

impl fmt::Display for NetworkName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkName::Custom(name) => write!(f, "{}", name),
            NetworkName::Dev => write!(f, "dev"),
        }
    }
}

impl FromStr for NetworkName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "dev" => Ok(NetworkName::Dev),
            other => Ok(NetworkName::Custom(other.to_string())),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Network {
    pub octez_node_rpc_endpoint: String,
    pub jstz_node_endpoint: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NetworkConfig {
    // if None, the users have to specify the network in the command
    #[serde(skip_serializing_if = "Option::is_none")]
    default_network: Option<NetworkName>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    networks: HashMap<String, Network>,
}

impl Config {
    /// Path to the configuration file
    pub fn path() -> PathBuf {
        jstz_home_dir().join("config.json")
    }

    pub fn reload(&mut self) -> Result<()> {
        *self = Self::load()?;
        Ok(())
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

    pub fn sandbox_logs_dir(&self) -> PathBuf {
        self.sandbox_logs_dir
            .clone()
            .unwrap_or(default_sandbox_logs_dir())
    }

    pub fn sandbox(&self) -> Result<&SandboxConfig> {
        self.sandbox.as_ref().ok_or(user_error!(
            "The sandbox is not running. Please run `jstz sandbox start`."
        ))
    }

    pub fn octez_client(
        &self,
        network_name: &Option<NetworkName>,
    ) -> Result<OctezClient> {
        let network = self.network(network_name)?;

        Ok(OctezClient {
            octez_setup: self.octez_setup.clone(),
            octez_client_dir: self.octez_client_dir(network_name)?,
            endpoint: network.octez_node_rpc_endpoint,
            disable_disclaimer: true,
        })
    }

    pub fn octez_client_sandbox(&self) -> Result<OctezClient> {
        self.octez_client(&Some(NetworkName::Dev))
    }

    pub fn jstz_client(&self, network_name: &Option<NetworkName>) -> Result<JstzClient> {
        let network = self.network(network_name)?;

        Ok(JstzClient::new(network.jstz_node_endpoint.clone()))
    }

    pub fn octez_node(&self) -> Result<OctezNode> {
        let sandbox = self.sandbox()?;

        Ok(OctezNode {
            octez_setup: self.octez_setup.clone(),
            octez_node_dir: sandbox.octez_node_dir.clone(),
        })
    }

    pub fn octez_rollup_node(
        &self,
        network_name: &Option<NetworkName>,
    ) -> Result<OctezRollupNode> {
        let sandbox = self.sandbox()?;

        let network = self.network(network_name)?;

        Ok(OctezRollupNode {
            octez_setup: self.octez_setup.clone(),
            octez_rollup_node_dir: sandbox.octez_rollup_node_dir.clone(),
            octez_client_dir: self.octez_client_dir(network_name)?,
            endpoint: network.octez_node_rpc_endpoint,
        })
    }

    pub fn octez_rollup_node_sandbox(&self) -> Result<OctezRollupNode> {
        self.octez_rollup_node(&Some(NetworkName::Dev))
    }

    fn octez_client_dir(
        &self,
        network_name: &Option<NetworkName>,
    ) -> Result<Option<PathBuf>> {
        let sandbox = self.sandbox()?;
        Ok(match self.network_name(network_name)? {
            NetworkName::Dev => Some(sandbox.octez_client_dir.clone()),
            _ => None,
        })
    }

    fn network(&self, name: &Option<NetworkName>) -> Result<Network> {
        let network = match name {
            Some(name) => self.lookup_network(name),
            None => {
                let name = self.networks.default_network.as_ref().ok_or_else(||user_error!(
                    "No default network found in the config file. Please specify the `--network` flag or set the default network in the config file."
                ))?;

                self.lookup_network(name)
            }
        };

        Ok(network?.clone())
    }

    pub fn network_name(&self, name: &Option<NetworkName>) -> Result<NetworkName> {
        match name {
            Some(name) => Ok(name.clone()),
            None => self.networks.default_network.clone().ok_or_else(|| {
                user_error!("No default network found in the config file. Please specify the `--network` flag or set the default network in the config file.")
            }),
        }
    }

    fn lookup_network(&self, name: &NetworkName) -> Result<Network> {
        match name {
            NetworkName::Custom(name) => {
                let network = self.networks.networks.get(name).ok_or_else(|| {
                    user_error!("Network '{}' not found in the config file.", name)
                })?;

                Ok(network.clone())
            }
            NetworkName::Dev => Ok(Network {
                octez_node_rpc_endpoint: format!(
                    "http://{}:{}",
                    SANDBOX_LOCAL_HOST_ADDR, SANDBOX_OCTEZ_NODE_RPC_PORT
                ),
                jstz_node_endpoint: format!(
                    "http://{}:{}",
                    SANDBOX_LOCAL_HOST_ADDR, SANDBOX_JSTZ_NODE_PORT,
                ),
            }),
        }
    }
}
