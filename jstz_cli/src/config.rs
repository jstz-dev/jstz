use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    root_dir: String,
    octez_client_dir: String,
    rpc: u16,
    url_aliases: HashMap<String, String>,
    name_aliases: HashMap<String, String>,
    tz4_aliases: HashMap<String, String>,
    is_sandbox_running: bool,
    active_pids: Vec<u32>,
}

impl Config {
    fn default() -> Self {
        Config {
            root_dir: "..".to_string(),
            octez_client_dir: "octez_client".to_string(),
            rpc: 18730,
            url_aliases: HashMap::new(),
            name_aliases: HashMap::new(),
            tz4_aliases: HashMap::new(),
            is_sandbox_running: false,
            active_pids: Vec::new(),
        }
    }

    // Path to the configuration file
    fn config_path() -> PathBuf {
        let mut path = dirs::home_dir().expect("Failed to get home directory");
        path.push(".jstz");
        path.push("sandbox.json");
        path
    }

    // Load the configuration from the file and update self with the loaded values
    pub fn load_from_file() -> Result<Self, std::io::Error> {
        let path = Self::config_path();

        let new_config = if !path.exists() {
            // If the file doesn't exist, create a default one
            let default_config = Config::default();
            default_config.save_to_file()?;
            default_config
        } else {
            let json = fs::read_to_string(&path)?;
            serde_json::from_str(&json)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?
        };

        // Replace the current instance with the new_config
        Ok(new_config)
    }

    // Save the configuration to the file
    pub fn save_to_file(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    // Getter and setter for root_dir
    pub fn get_root_dir(&self) -> &String {
        &self.root_dir
    }

    // pub fn set_root_dir(&mut self, value: String) {
    //     self.root_dir = value;
    // }

    // // Getter and setter for octez_client_dir
    pub fn get_octez_client_dir(&self) -> &String {
        &self.octez_client_dir
    }

    pub fn set_octez_client_dir(&mut self, value: String) {
        self.octez_client_dir = value;
    }

    // // Getter and setter for rpc
    // pub fn get_rpc(&self) -> u16 {
    //     self.rpc
    // }

    // pub fn set_rpc(&mut self, value: u16) {
    //     self.rpc = value;
    // }

    pub fn get_octez_client_path(&self) -> String {
        let octez_client_path = format!("{}/octez-client", self.root_dir);
        octez_client_path
    }

    pub fn get_octez_client_setup_args(&self) -> Vec<String> {
        let args = vec![
            "-base-dir".to_string(),
            self.octez_client_dir.clone(),
            "-endpoint".to_string(),
            format!("http://127.0.0.1:{}", self.rpc),
        ];
        args
    }

    pub fn octez_client_command(&self) -> Command {
        let mut cmd = Command::new(self.get_octez_client_path());
        cmd.args(self.get_octez_client_setup_args());
        cmd
    }

    pub fn octez_node_command(&self) -> Command {
        let cmd = Command::new("../octez-node");
        cmd
    }

    pub fn octez_rollup_node_command(&self) -> Command {
        let mut cmd = Command::new("../octez-smart-rollup-node");
        cmd.args(self.get_octez_client_setup_args());
        cmd
    }

    // Methods for url_aliases
    // pub fn get_url_alias(&self, alias: &str) -> Option<String> {
    //     self.url_aliases.get(alias).cloned()
    // }

    // pub fn set_url_alias(&mut self, alias: String, value: String) {
    //     self.url_aliases.insert(alias, value);
    // }

    // pub fn remove_url_alias(&mut self, alias: &str) {
    //     self.url_aliases.remove(alias);
    // }

    // Methods for name_aliases
    pub fn get_name_alias(&self, alias: &str) -> Option<String> {
        self.name_aliases.get(alias).cloned()
    }

    // pub fn set_name_alias(&mut self, alias: String, value: String) {
    //     self.name_aliases.insert(alias, value);
    // }

    // pub fn remove_name_alias(&mut self, alias: &str) {
    //     self.name_aliases.remove(alias);
    // }

    // // Methods for tz4_aliases
    pub fn get_tz4_alias(&self, alias: &str) -> Option<String> {
        self.tz4_aliases.get(alias).cloned()
    }

    // pub fn set_tz4_alias(&mut self, alias: String, value: String) {
    //     self.tz4_aliases.insert(alias, value);
    // }

    // pub fn remove_tz4_alias(&mut self, alias: &str) {
    //     self.tz4_aliases.remove(alias);
    // }

    // Getter and setter for is_sandbox_running
    pub fn get_is_sandbox_running(&self) -> bool {
        self.is_sandbox_running
    }

    pub fn set_is_sandbox_running(&mut self, value: bool) {
        self.is_sandbox_running = value;
    }

    // Methods for active_pids
    pub fn get_active_pids(&self) -> Vec<u32> {
        self.active_pids.clone()
    }

    pub fn add_pid(&mut self, pid: u32) {
        self.active_pids.push(pid);
    }

    pub fn remove_pid(&mut self, pid: u32) {
        self.active_pids.retain(|&x| x != pid);
    }
}
