use std::path::PathBuf;

use jstz_node::config::RunModeType;
use serde::Deserialize;
use tezos_crypto_rs::hash::SmartRollupHash;

/// A subset of JstzNodeConfig that is exposed to users.
#[derive(Deserialize, Default, PartialEq, Debug, Clone)]
pub(crate) struct UserJstzNodeConfig {
    #[serde(default)]
    /// Flag indicating if Jstz node should not be launched.
    pub skipped: bool,
    pub mode: Option<RunModeType>,
    pub capacity: Option<usize>,
    pub debug_log_file: Option<PathBuf>,
    pub riscv_kernel_path: Option<PathBuf>,
    pub rollup_address: Option<SmartRollupHash>,
    #[serde(default)]
    pub storage_sync: bool,
}

/// Oracle node config for jstzd.
#[derive(Deserialize, Default, Clone)]
pub(crate) struct UserOracleNodeConfig {
    #[serde(default)]
    /// Flag indicating if oracle node should not be launched.
    pub skipped: bool,
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use jstz_node::config::RunModeType;
    use tezos_crypto_rs::hash::SmartRollupHash;

    use crate::user_config::UserOracleNodeConfig;

    use super::UserJstzNodeConfig;

    #[test]
    fn user_jstz_node_config() {
        assert_eq!(
            UserJstzNodeConfig::default(),
            UserJstzNodeConfig {
                mode: None,
                capacity: None,
                debug_log_file: None,
                riscv_kernel_path: None,
                rollup_address: None,
                storage_sync: false,
                skipped: false
            }
        )
    }

    #[test]
    fn deserialise_user_jstz_node_config() {
        let s = r#"{
            "skipped": true,
            "mode": "sequencer",
            "capacity": 42,
            "debug_log_file": "/tmp/log",
            "riscv_kernel_path": "/riscv/kernel",
            "rollup_address": "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
            "storage_sync": true
        }"#;
        let config = serde_json::from_str::<UserJstzNodeConfig>(s).unwrap();
        let expected = UserJstzNodeConfig {
            skipped: true,
            mode: Some(RunModeType::Sequencer),
            capacity: Some(42),
            debug_log_file: Some(PathBuf::from_str("/tmp/log").unwrap()),
            riscv_kernel_path: Some(PathBuf::from_str("/riscv/kernel").unwrap()),
            rollup_address: Some(
                SmartRollupHash::from_base58_check(
                    "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
                )
                .unwrap(),
            ),
            storage_sync: true,
        };
        assert_eq!(config, expected);

        let s = r#"{"skipped": false, "mode": "sequencer", "capacity": 10}"#;
        let config = serde_json::from_str::<UserJstzNodeConfig>(s).unwrap();
        let expected = UserJstzNodeConfig {
            skipped: false,
            mode: Some(RunModeType::Sequencer),
            capacity: Some(10),
            ..Default::default()
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialise_user_octez_node_config() {
        let s = r#"{"skipped": true}"#;
        let config = serde_json::from_str::<UserOracleNodeConfig>(s).unwrap();
        assert!(config.skipped);

        let s = r#"{}"#;
        let config = serde_json::from_str::<UserOracleNodeConfig>(s).unwrap();
        assert!(!config.skipped);
    }
}
