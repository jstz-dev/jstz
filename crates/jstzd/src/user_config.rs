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

#[cfg(test)]
mod tests {
    use jstz_node::config::RunModeType;

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
    fn deserialise_jstz_node_config_wrapper() {
        let s = r#"{"mode": "sequencer", "capacity": 10}"#;
        let config = serde_json::from_str::<UserJstzNodeConfig>(s).unwrap();
        let expected = UserJstzNodeConfig {
            skipped: false,
            mode: Some(RunModeType::Sequencer),
            capacity: Some(10),
            ..Default::default()
        };
        assert_eq!(config, expected);

        let s = r#"{"skipped": true, "mode": "sequencer", "capacity": 10}"#;
        let config = serde_json::from_str::<UserJstzNodeConfig>(s).unwrap();
        let expected = UserJstzNodeConfig {
            skipped: true,
            mode: Some(RunModeType::Sequencer),
            capacity: Some(10),
            ..Default::default()
        };
        assert_eq!(config, expected);

        let s = r#"{"skipped": true}"#;
        let config = serde_json::from_str::<UserJstzNodeConfig>(s).unwrap();
        let expected = UserJstzNodeConfig {
            skipped: true,
            ..Default::default()
        };
        assert_eq!(config, expected);

        let s = r#"{"skipped": true, "config": {"mode": "sequencer", "capacity": 10}}"#;
        let err = serde_json::from_str::<UserJstzNodeConfig>(s)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("unknown field `config`"),
            "error string '{err}' does not contain the expected string"
        );
    }
}
