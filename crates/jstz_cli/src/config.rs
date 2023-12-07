use std::{
    collections::HashMap,
    fs,
    io::{Error, ErrorKind},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use jstz_proto::context::account::Address;
use octez::{OctezClient, OctezNode, OctezRollupNode};
use serde::{Deserialize, Serialize};

use crate::{
    account::account::{Account, AliasAccount},
    jstz::JstzClient,
};

fn home() -> PathBuf {
    dirs::home_dir()
        .expect("Failed to get home directory")
        .join(".jstz")
}

// Represents a collection of accounts
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AccountConfig {
    pub current_alias: Option<String>,
    accounts: HashMap<String, Account>,
}

impl AccountConfig {
    pub fn add_alias(&mut self, alias: String, address: String) -> Result<()> {
        if self.contains(alias.as_str()) {
            return Err(anyhow!("Account already exists"));
        }

        let account = AliasAccount::new(address.clone(), alias.clone())?;

        self.upsert(account);

        Ok(())
    }

    pub fn contains(&self, alias: &str) -> bool {
        self.accounts.contains_key(alias)
    }

    pub fn upsert<T: Into<Account>>(&mut self, account: T) {
        let account = account.into();
        self.accounts.insert(account.alias().to_string(), account);
    }

    pub fn alias_or_current(&self, alias: Option<String>) -> Result<String> {
        alias
            .or(self.current_alias.clone())
            .ok_or(anyhow!("No account selected!"))
    }

    pub fn get_address_from(&self, alias: Option<String>) -> Result<String> {
        match alias {
            Some(value) if value.starts_with("tz1") => Ok(value),
            _ => Ok(self
                .account_or_current(alias)?
                .address()
                .to_base58()
                .to_string()),
        }
    }

    pub fn get(&self, alias: &str) -> Result<&Account> {
        self.accounts.get(alias).ok_or(anyhow!("Account not found"))
    }

    pub fn get_mut(&mut self, alias: &str) -> Result<&mut Account> {
        self.accounts
            .get_mut(alias)
            .ok_or(anyhow!("Account not found"))
    }

    pub fn account_or_current(&self, alias: Option<String>) -> Result<&Account> {
        let alias = self.alias_or_current(alias)?;

        self.get(&alias)
    }

    pub fn account_or_current_mut(
        &mut self,
        alias: Option<String>,
    ) -> Result<&mut Account> {
        let alias = self.alias_or_current(alias)?;

        self.get_mut(&alias)
    }

    pub fn remove(&mut self, alias: &String) -> Option<Account> {
        if self.current_alias == Some(alias.clone()) {
            self.current_alias = None;
        }
        self.accounts.remove(alias)
    }

    pub fn list_all(&self) -> Vec<(&String, &Account)> {
        self.accounts.iter().collect()
    }

    pub fn get_address(&self, address_or_alias: &str) -> Result<Address> {
        if let Ok(address) = Address::from_base58(address_or_alias) {
            return Ok(address);
        }

        if let Ok(account) = self.get(address_or_alias) {
            return Ok(account.address().clone());
        }

        Err(anyhow!("Invalid alias or address"))
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    /// Path to `jstz` directory
    pub jstz_path: PathBuf,
    /// Path to octez installation
    pub octez_path: PathBuf,
    /// Sandbox config (None if sandbox is not running)
    pub sandbox: Option<SandboxConfig>,
    /// List of accounts
    pub accounts: AccountConfig,
}

pub const SANDBOX_OCTEZ_SMART_ROLLUP_PORT: u16 = 8932;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SandboxConfig {
    /// Directory of the octez client (initialized when sandbox is running)
    pub octez_client_dir: PathBuf,
    /// Directory of the octez node
    pub octez_node_dir: PathBuf,
    /// Directory of the octez rollup node
    pub octez_rollup_node_dir: PathBuf,
    /// The port of the octez node
    pub octez_node_port: u16,
    /// The port of the octez RPC node
    pub octez_node_rpc_port: u16,
    /// The port of the jstz node
    pub jstz_node_port: u16,
    /// Pid of the pid
    pub pid: u32,
    private: (),
}

impl SandboxConfig {
    pub fn new(
        pid: u32,
        octez_client_dir: PathBuf,
        octez_node_dir: PathBuf,
        octez_rollup_node_dir: PathBuf,
    ) -> Self {
        Self {
            octez_client_dir,
            octez_node_dir,
            octez_rollup_node_dir,
            pid,
            octez_node_port: 18731,
            octez_node_rpc_port: 18730,
            jstz_node_port: 8933,
            private: (),
        }
    }
}

impl Config {
    /// Path to the configuration file
    pub fn path() -> PathBuf {
        home().join("config.json")
    }

    /// Load the configuration from the file
    pub fn load() -> std::io::Result<Self> {
        let path = Self::path();

        let config = if path.exists() {
            let json = fs::read_to_string(&path)?;
            serde_json::from_str(&json)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?
        } else {
            Config::default()
        };

        Ok(config)
    }

    /// Save the configuration to the file
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn sandbox(&self) -> Result<&SandboxConfig> {
        self.sandbox
            .as_ref()
            .ok_or(anyhow!("Sandbox is not running"))
    }

    pub fn accounts(&mut self) -> &mut AccountConfig {
        &mut self.accounts
    }

    pub fn octez_client(&self) -> Result<OctezClient> {
        let sandbox = self.sandbox()?;

        Ok(OctezClient {
            octez_client_bin: Some(self.octez_path.join("octez-client")),
            octez_client_dir: sandbox.octez_client_dir.clone(),
            endpoint: format!("http://127.0.0.1:{}", sandbox.octez_node_rpc_port),
            disable_disclaimer: true,
        })
    }

    pub fn jstz_client(&self) -> Result<JstzClient> {
        let sandbox = self.sandbox()?;

        Ok(JstzClient::new(format!(
            "http://127.0.0.1:{}",
            sandbox.jstz_node_port
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
            endpoint: format!("http://127.0.0.1:{}", sandbox.octez_node_rpc_port),
        })
    }
}
