use derive_more::{From, TryInto};
use jstz_crypto::{
    public_key::PublicKey, public_key_hash::PublicKeyHash, secret_key::SecretKey,
    smart_function_hash::SmartFunctionHash,
};
use jstz_proto::context::account::Address;
use log::debug;
use octez::OctezClient;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    collections::{hash_map, HashMap},
    fmt, fs,
    path::PathBuf,
    str::FromStr,
};

use crate::{
    error::{bail, user_error, Result},
    jstz::JstzClient,
    sandbox::{
        JSTZD_SERVER_BASE_URL, SANDBOX_JSTZ_NODE_PORT, SANDBOX_LOCAL_HOST_ADDR,
        SANDBOX_OCTEZ_NODE_RPC_PORT,
    },
    utils::AddressOrAlias,
};

#[cfg(not(test))]
pub fn jstz_home_dir() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(value)
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".config/jstz")
    }
}

#[cfg(test)]
pub fn jstz_home_dir() -> PathBuf {
    use tempfile::env::temp_dir;

    temp_dir()
}

// Represents a collection of accounts: users or smart functions
#[derive(Serialize, Deserialize, Debug, Clone, From, TryInto)]
pub enum Account {
    User(User),
    SmartFunction(SmartFunction),
}

impl Account {
    pub fn address(&self) -> Address {
        match self {
            Account::User(user) => user.address.clone().into(),
            Account::SmartFunction(sf) => sf.address.clone().into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub address: PublicKeyHash,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SmartFunction {
    pub address: SmartFunctionHash,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AccountConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_alias: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub accounts: HashMap<String, Account>,
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

                Ok(account.address())
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
                .map(|(_, user)| user.address.clone().into()),
        }
    }

    pub fn resolve_l1(
        &self,
        cfg: &Config,
        network: &Option<NetworkName>,
    ) -> Result<Address> {
        match self {
            AddressOrAlias::Address(address) => Ok(address.clone()),
            AddressOrAlias::Alias(alias) => {
                let alias_info = cfg
                    .octez_client(network)?
                    .alias_info(alias)
                    .map_err(|_|
                        user_error!(
                        "Alias '{}' not found in octez-client. Please provide a valid address or alias.",
                        alias
                    ))?;

                let address = Address::from_base58(&alias_info.address)
                    .map_err(|e| user_error!("{}", e))?;

                Ok(address)
            }
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

// A subset of jstzd::task::jstzd::JstzdConfig
#[derive(Deserialize, Debug, Clone)]
pub struct JstzdConfig {
    pub octez_node: OctezNodeConfig,
    #[allow(unused)]
    pub octez_client: OctezClientConfig,
    pub jstz_node: JstzNodeConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OctezClientConfig {
    #[allow(unused)]
    pub base_dir: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OctezNodeConfig {
    pub rpc_endpoint: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JstzNodeConfig {
    pub endpoint: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    /// Path to octez installation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub octez_path: Option<PathBuf>,
    /// The octez client directory to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub octez_client_dir: Option<PathBuf>,
    /// List of accounts
    #[serde(flatten)]
    pub accounts: AccountConfig,
    /// Available networks
    #[serde(flatten)]
    pub networks: NetworkConfig,
    /// Sandbox logs dir
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_logs_dir: Option<PathBuf>,
    #[serde(skip)]
    pub jstzd_config: Option<JstzdConfig>,
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
            NetworkName::Custom(name) => write!(f, "{name}"),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Network {
    pub octez_node_rpc_endpoint: String,
    pub jstz_node_endpoint: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NetworkConfig {
    // if None, the users have to specify the network in the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_network: Option<NetworkName>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub networks: HashMap<String, Network>,
}

impl Config {
    pub fn new(
        octez_client_dir: Option<PathBuf>,
        accounts: AccountConfig,
        networks: NetworkConfig,
    ) -> Self {
        Self {
            octez_path: None,
            octez_client_dir,
            accounts,
            networks,
            sandbox_logs_dir: None,
            jstzd_config: None,
        }
    }

    /// Path to the default configuration file in the home directory
    pub fn default_path() -> PathBuf {
        jstz_home_dir().join("config.json")
    }

    pub async fn reload(&mut self) -> Result<()> {
        self.reload_path(None).await
    }

    pub async fn reload_path(&mut self, config_path: Option<PathBuf>) -> Result<()> {
        *self = Self::load_path(config_path).await?;
        Ok(())
    }

    /// Load the configuration from the default config file
    pub async fn load() -> Result<Self> {
        Self::load_path(None).await
    }

    pub async fn load_path(config_path: Option<PathBuf>) -> Result<Self> {
        let path = config_path.unwrap_or_else(Self::default_path);

        let mut config = if path.exists() {
            let json = fs::read_to_string(&path)?;
            debug!("Config file contents: {}", json);

            serde_json::from_str(&json).map_err(|_| {
                user_error!("Your configuration file is improperly configured.")
            })?
        } else {
            Config::default()
        };

        let _ = config.load_jstzd_config().await;
        debug!("Config (on load): {:?}", config);

        Ok(config)
    }

    async fn load_jstzd_config(&mut self) -> Result<()> {
        self.jstzd_config
            .replace(Self::fetch_jstzd_config(JSTZD_SERVER_BASE_URL).await?);
        Ok(())
    }

    async fn fetch_jstzd_config(jstzd_server_base_url: &str) -> Result<JstzdConfig> {
        let r = reqwest::Client::new();
        let c = r
            .get(format!("{jstzd_server_base_url}/config/"))
            .send()
            .await?
            .json::<JstzdConfig>()
            .await?;
        Ok(c)
    }

    pub fn save_to_path(&self, config_path: Option<PathBuf>) -> Result<()> {
        debug!("Config (on save): {:?}", self);

        let path = config_path.unwrap_or_else(Self::default_path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;

        Ok(())
    }

    /// Save the configuration to the default config file
    pub fn save(&self) -> Result<()> {
        self.save_to_path(None)
    }

    pub fn octez_client(
        &self,
        network_name: &Option<NetworkName>,
    ) -> Result<OctezClient> {
        let network = self.network(network_name)?;

        Ok(OctezClient {
            octez_client_bin: self
                .octez_path
                .as_ref()
                .map(|path| path.join("octez-client")),
            octez_client_dir: self.octez_client_dir.clone(),
            endpoint: network.octez_node_rpc_endpoint,
            disable_disclaimer: true,
        })
    }

    pub fn jstz_client(&self, network_name: &Option<NetworkName>) -> Result<JstzClient> {
        let network = self.network(network_name)?;

        Ok(JstzClient::new(network.jstz_node_endpoint.clone()))
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
            NetworkName::Dev => match self.jstzd_config.as_ref() {
                Some(jstzd_config) => {
                    // Assuming that when jstzd config is available, the cli is being used
                    // against jstzd, so we take values from jstzd config
                    Ok(Network {
                        octez_node_rpc_endpoint: jstzd_config
                            .octez_node
                            .rpc_endpoint
                            .clone(),
                        jstz_node_endpoint: jstzd_config.jstz_node.endpoint.clone(),
                    })
                }
                None => Ok(Network {
                    octez_node_rpc_endpoint: format!(
                        "http://{SANDBOX_LOCAL_HOST_ADDR}:{SANDBOX_OCTEZ_NODE_RPC_PORT}"
                    ),
                    jstz_node_endpoint: format!(
                        "http://{SANDBOX_LOCAL_HOST_ADDR}:{SANDBOX_JSTZ_NODE_PORT}",
                    ),
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        Config, JstzNodeConfig, JstzdConfig, Network, NetworkConfig, NetworkName,
        OctezClientConfig, OctezNodeConfig,
    };

    fn dummy_jstzd_config() -> JstzdConfig {
        JstzdConfig {
            octez_client: OctezClientConfig {
                base_dir: "/base".to_owned(),
            },
            octez_node: OctezNodeConfig {
                rpc_endpoint: "http://octez.node.endpoint/".to_owned(),
            },
            jstz_node: JstzNodeConfig {
                endpoint: "http://jstz.node.endpoint/".to_owned(),
            },
        }
    }

    #[test]
    fn lookup_network_with_jstzd() {
        let mut config = Config::default();
        config.jstzd_config.replace(dummy_jstzd_config());
        let jstzd_config = config.jstzd_config.as_ref().unwrap();
        assert_eq!(
            config.lookup_network(&NetworkName::Dev).unwrap(),
            Network {
                octez_node_rpc_endpoint: jstzd_config.octez_node.rpc_endpoint.clone(),
                jstz_node_endpoint: jstzd_config.jstz_node.endpoint.clone()
            }
        )
    }

    #[test]
    fn lookup_network_dev() {
        let config = Config::default();
        assert_eq!(
            config.lookup_network(&NetworkName::Dev).unwrap(),
            Network {
                octez_node_rpc_endpoint: "http://127.0.0.1:18730".to_owned(),
                jstz_node_endpoint: "http://127.0.0.1:8933".to_owned()
            }
        )
    }

    #[test]
    fn lookup_network_custom() {
        let dummy_network = Network {
            octez_node_rpc_endpoint: "a".to_owned(),
            jstz_node_endpoint: "b".to_owned(),
        };
        let config = Config {
            networks: NetworkConfig {
                networks: HashMap::from([("foo".to_owned(), dummy_network.clone())]),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            config
                .lookup_network(&NetworkName::Custom("foo".to_string()))
                .unwrap(),
            dummy_network
        );
        assert_eq!(
            config
                .lookup_network(&NetworkName::Custom("bar".to_string()))
                .unwrap_err()
                .to_string(),
            "Network 'bar' not found in the config file."
        );
    }

    #[tokio::test]
    async fn fetch_jstzd_config_invalid_config() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/config/").with_body("{}").create();

        assert_eq!(
            Config::fetch_jstzd_config(&server.url())
                .await
                .unwrap_err()
                .to_string(),
            "error decoding response body: missing field `octez_node` at line 1 column 2"
        );
    }

    #[tokio::test]
    async fn fetch_jstzd_config_failed_to_send_request() {
        assert_eq!(
            Config::fetch_jstzd_config("")
                .await
                .unwrap_err()
                .to_string(),
            "builder error: relative URL without a base"
        );
    }

    #[tokio::test]
    async fn fetch_jstzd_config_ok() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/config/").with_body(r#"{"octez_node":{"rpc_endpoint":"foo"},"octez_client":{"base_dir":"bar"},"jstz_node":{"endpoint":"baz"}}"#).create();
        let cfg = Config::fetch_jstzd_config(&server.url()).await.unwrap();
        assert_eq!(cfg.octez_node.rpc_endpoint, "foo");
        assert_eq!(cfg.octez_client.base_dir, "bar");
        assert_eq!(cfg.jstz_node.endpoint, "baz");
    }
}
