use std::{
    collections::HashMap,
    fs,
    io::{Error, ErrorKind},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::account::account::Account;

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
    pub fn upsert(&mut self, account: Account) {
        self.accounts.insert(account.alias.clone(), account);
    }

    pub fn alias_or_current(&self, alias: Option<String>) -> Result<String> {
        alias
            .or(self.current_alias.clone())
            .ok_or(anyhow!("No account selected!"))
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
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    /// Path to `jstz` directory
    pub jstz_path: PathBuf,
    /// Path to octez installation
    pub octez_path: PathBuf,
    /// The port of the octez node
    pub octez_node_port: u16,
    /// The port of the octez RPC node
    pub octez_node_rpc_port: u16,
    /// Sandbox config (None if sandbox is not running)
    pub sandbox: Option<SandboxConfig>,
    /// List of accounts
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
            private: (),
        }
    }
}

impl Config {
    fn default() -> Self {
        Config {
            jstz_path: PathBuf::from_str(".").unwrap(),
            octez_path: PathBuf::from_str(".").unwrap(),
            octez_node_port: 18731,
            octez_node_rpc_port: 18730,
            sandbox: None,
            accounts: AccountConfig::default(),
        }
    }

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
}
