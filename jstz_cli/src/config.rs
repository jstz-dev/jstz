use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use std::io::{Error, ErrorKind};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
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

    // Load the configuration from the file
    pub fn load_from_file() -> Result<Self, std::io::Error> {
        let path = Self::config_path();

        if !path.exists() {
            // If the file doesn't exist, create a default one
            let default_config = Config::default();
            default_config.save_to_file()?;
            return Ok(default_config);
        }

        let json = fs::read_to_string(&path)?;
        serde_json::from_str(&json).map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    // Save the configuration to the file
    fn save_to_file(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    // Getter and setter for root_dir
    pub fn get_root_dir(&self) -> Result<String, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.root_dir)
    }

    pub fn set_root_dir(&mut self, value: String) -> Result<(), std::io::Error> {
        self.root_dir = value;
        self.save_to_file()
    }

    // Getter and setter for octez_client_dir
    pub fn get_octez_client_dir(&self) -> Result<String, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.octez_client_dir)
    }

    pub fn set_octez_client_dir(&mut self, value: String) -> Result<(), std::io::Error> {
        self.octez_client_dir = value;
        self.save_to_file()
    }

    // Getter and setter for rpc
    pub fn get_rpc(&self) -> Result<u16, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.rpc)
    }

    pub fn set_rpc(&mut self, value: u16) -> Result<(), std::io::Error> {
        self.rpc = value;
        self.save_to_file()
    }

    pub fn get_octez_client_path(&self) -> Result<String, std::io::Error> {
        let config = Self::load_from_file()?;
        let octez_client_path = format!("{}/octez-client", config.root_dir);
        Ok(octez_client_path)
    }

    pub fn get_octez_client_setup_args(&self) -> Result<Vec<String>, std::io::Error> {
        let config = Self::load_from_file()?;
        let args = vec![
            "-base-dir".to_string(),
            config.octez_client_dir.clone(),
            "-endpoint".to_string(),
            format!("http://127.0.0.1:{}", config.rpc),
        ];
        Ok(args)
    }

    // Methods for url_aliases
    pub fn get_url_alias(&self, alias: &str) -> Result<Option<String>, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.url_aliases.get(alias).cloned())
    }

    pub fn set_url_alias(&mut self, alias: String, value: String) -> Result<(), std::io::Error> {
        self.url_aliases.insert(alias, value);
        self.save_to_file()
    }

    pub fn remove_url_alias(&mut self, alias: &str) -> Result<(), std::io::Error> {
        self.url_aliases.remove(alias);
        self.save_to_file()
    }

    // Methods for name_aliases
    pub fn get_name_alias(&self, alias: &str) -> Result<Option<String>, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.name_aliases.get(alias).cloned())
    }

    pub fn set_name_alias(&mut self, alias: String, value: String) -> Result<(), std::io::Error> {
        self.name_aliases.insert(alias, value);
        self.save_to_file()
    }

    pub fn remove_name_alias(&mut self, alias: &str) -> Result<(), std::io::Error> {
        self.name_aliases.remove(alias);
        self.save_to_file()
    }

    // Methods for tz4_aliases
    pub fn get_tz4_alias(&self, alias: &str) -> Result<Option<String>, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.tz4_aliases.get(alias).cloned())
    }

    pub fn set_tz4_alias(&mut self, alias: String, value: String) -> Result<(), std::io::Error> {
        self.tz4_aliases.insert(alias, value);
        self.save_to_file()
    }

    pub fn remove_tz4_alias(&mut self, alias: &str) -> Result<(), std::io::Error> {
        self.tz4_aliases.remove(alias);
        self.save_to_file()
    }

    // Getter and setter for is_sandbox_running
    pub fn get_is_sandbox_running(&self) -> Result<bool, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.is_sandbox_running)
    }

    pub fn set_is_sandbox_running(&mut self, value: bool) -> Result<(), std::io::Error> {
        self.is_sandbox_running = value;
        self.save_to_file()
    }

    // Methods for active_pids
    pub fn get_active_pids(&self) -> Result<Vec<u32>, std::io::Error> {
        let config = Self::load_from_file()?;
        Ok(config.active_pids.clone())
    }

    pub fn add_pid(&mut self, pid: u32) -> Result<(), std::io::Error> {
        self.active_pids.push(pid);
        self.save_to_file()
    }

    pub fn remove_pid(&mut self, pid: u32) -> Result<(), std::io::Error> {
        self.active_pids.retain(|&x| x != pid);
        self.save_to_file()
    }
}

