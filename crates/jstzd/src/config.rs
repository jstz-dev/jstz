#![allow(dead_code)]

use anyhow::{Context, Result};
use octez::r#async::{
    baker::OctezBakerConfigBuilder, client::OctezClientConfigBuilder,
    node_config::OctezNodeConfigBuilder, protocol::ProtocolParameterBuilder,
};
use serde::Deserialize;
use tokio::io::AsyncReadExt;

#[derive(Deserialize, Default)]
struct Config {
    server_port: Option<u16>,
    #[serde(default)]
    octez_node: OctezNodeConfigBuilder,
    #[serde(default)]
    octez_baker: OctezBakerConfigBuilder,
    octez_client: Option<OctezClientConfigBuilder>,
    #[serde(default)]
    protocol: ProtocolParameterBuilder,
}

async fn parse_config(path: &str) -> Result<Config> {
    let mut s = String::new();
    tokio::fs::File::open(path)
        .await
        .context("failed to open config file")?
        .read_to_string(&mut s)
        .await
        .context("failed to read config file")?;
    Ok(serde_json::from_str::<Config>(&s)?)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::PathBuf, str::FromStr};

    use http::Uri;
    use octez::r#async::{
        baker::{BakerBinaryPath, OctezBakerConfigBuilder},
        client::OctezClientConfigBuilder,
        endpoint::Endpoint,
        node_config::{
            OctezNodeConfigBuilder, OctezNodeHistoryMode, OctezNodeRunOptionsBuilder,
        },
        protocol::{
            BootstrapAccount, BootstrapContract, BootstrapSmartRollup, Protocol,
            ProtocolConstants, ProtocolParameterBuilder, SmartRollupPvmKind,
        },
    };
    use tempfile::NamedTempFile;

    use super::Config;

    #[tokio::test]
    async fn parse_config() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = serde_json::to_string(
            &serde_json::json!({"octez_client": {"octez_node_endpoint": "localhost:8888"}}),
        )
        .unwrap();
        tmp_file.write_all(content.as_bytes()).unwrap();

        let config = super::parse_config(&tmp_file.path().to_string_lossy())
            .await
            .unwrap();
        assert_eq!(
            config.octez_client,
            Some(OctezClientConfigBuilder::new(Endpoint::localhost(8888)))
        );
    }

    #[test]
    fn deserialize_config_default() {
        let config = serde_json::from_value::<Config>(serde_json::json!({})).unwrap();
        assert_eq!(config.octez_baker, OctezBakerConfigBuilder::default());
        assert!(config.octez_client.is_none());
        assert_eq!(config.octez_node, OctezNodeConfigBuilder::default());
        assert_eq!(config.protocol, ProtocolParameterBuilder::default());
        assert!(config.server_port.is_none());
    }

    #[test]
    fn deserialize_config_octez_node() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "octez_node": {
                "binary_path": "bin",
                "data_dir": "data_dir",
                "network": "test",
                "rpc_endpoint": "rpc.test",
                "p2p_address": "p2p.test",
                "log_file": "log_file",
                "run_options": {
                    "synchronisation_threshold": 1,
                    "network": "test",
                    "history_mode": "archive"
                }
            }
        }))
        .unwrap();
        let mut expected = OctezNodeConfigBuilder::new();
        expected
            .set_binary_path("bin")
            .set_data_dir("data_dir")
            .set_network("test")
            .set_rpc_endpoint(&Endpoint::try_from(Uri::from_static("rpc.test")).unwrap())
            .set_p2p_address(&Endpoint::try_from(Uri::from_static("p2p.test")).unwrap())
            .set_log_file("log_file")
            .set_run_options(
                &OctezNodeRunOptionsBuilder::new()
                    .set_history_mode(OctezNodeHistoryMode::Archive)
                    .set_network("test")
                    .set_synchronisation_threshold(1)
                    .build(),
            );
        assert_eq!(config.octez_node, expected);
    }

    #[test]
    fn deserialize_config_octez_client() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "octez_client": {
                "binary_path": "bin",
                "base_dir": "base_dir",
                "disable_unsafe_disclaimer": false,
                "octez_node_endpoint": "rpc.test",
            }
        }))
        .unwrap();
        let expected = OctezClientConfigBuilder::new(
            Endpoint::try_from(Uri::from_static("rpc.test")).unwrap(),
        )
        .set_binary_path(PathBuf::from_str("bin").unwrap())
        .set_base_dir(PathBuf::from_str("base_dir").unwrap())
        .set_disable_unsafe_disclaimer(false);
        assert_eq!(config.octez_client, Some(expected));
    }

    #[test]
    fn deserialize_config_baker() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "octez_baker": {
                "binary_path": "bin",
                "octez_client_base_dir": "base_dir",
                "octez_node_endpoint": "rpc.test",
            }
        }))
        .unwrap();
        let expected = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Custom(PathBuf::from_str("bin").unwrap()))
            .set_octez_client_base_dir("base_dir")
            .set_octez_node_endpoint(
                &Endpoint::try_from(Uri::from_static("rpc.test")).unwrap(),
            );
        assert_eq!(config.octez_baker, expected);
    }

    #[test]
    fn deserialize_config_protocol() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "protocol": {
                "protocol": "parisC",
                "constants": "sandbox",
                "bootstrap_accounts": [["edpktkhoky4f5kqm2EVwYrMBq5rY9sLYdpFgXixQDWifuBHjhuVuNN", "1"]],
                "bootstrap_contracts": [{"amount":"1", "script": "dummy-script-no-hash"}],
                "bootstrap_smart_rollups": [{
                    "address": "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
                    "pvm_kind": "riscv",
                    "kernel": "dummy-kernel",
                    "parameters_ty": "dummy-params"
                }]
            }
        }))
        .unwrap();
        let mut expected = ProtocolParameterBuilder::new();
        expected
            .set_protocol(Protocol::ParisC)
            .set_constants(ProtocolConstants::Sandbox)
            .set_bootstrap_accounts([BootstrapAccount::new(
                "edpktkhoky4f5kqm2EVwYrMBq5rY9sLYdpFgXixQDWifuBHjhuVuNN",
                1,
            )
            .unwrap()])
            .set_bootstrap_contracts([BootstrapContract::new(
                serde_json::json!("dummy-script-no-hash"),
                1,
                None,
            )
            .unwrap()])
            .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
                "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK",
                SmartRollupPvmKind::Riscv,
                "dummy-kernel",
                serde_json::json!("dummy-params"),
            )
            .unwrap()]);
        assert_eq!(config.protocol, expected);
    }

    #[test]
    fn deserialize_config_port() {
        let config =
            serde_json::from_value::<Config>(serde_json::json!({"server_port":5678}))
                .unwrap();
        assert_eq!(config.server_port, Some(5678));
    }
}
