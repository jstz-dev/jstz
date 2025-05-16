use octez::r#async::node_config::{OctezNodeHistoryMode, OctezNodeRunOptionsBuilder};
use rust_embed::Embed;

use crate::task::jstzd::JstzdConfig;
use crate::{
    jstz_rollup_path, EXCHANGER_ADDRESS, JSTZ_NATIVE_BRIDGE_ADDRESS, JSTZ_ROLLUP_ADDRESS,
};
use anyhow::{Context, Result};
use http::Uri;
use jstz_node::config::JstzNodeConfig;
use jstz_node::config::KeyPair;
use octez::r#async::endpoint::Endpoint;
use octez::r#async::protocol::{
    BootstrapContract, BootstrapSmartRollup, ProtocolParameter, SmartRollupPvmKind,
};
use octez::r#async::{
    baker::{BakerBinaryPath, OctezBakerConfig, OctezBakerConfigBuilder},
    client::{OctezClientConfig, OctezClientConfigBuilder},
    file::FileWrapper,
    node_config::{OctezNodeConfig, OctezNodeConfigBuilder},
    protocol::{BootstrapAccount, ProtocolParameterBuilder},
    rollup::{OctezRollupConfigBuilder, RollupDataDir},
};
use serde::Deserialize;
use tezos_crypto_rs::hash::SmartRollupHash;
use tokio::io::AsyncReadExt;

const DEFAULT_JSTZD_SERVER_PORT: u16 = 54321;
const DEFAULT_JSTZ_NODE_ENDPOINT: &str = "0.0.0.0:8933";
pub const BOOTSTRAP_CONTRACT_NAMES: [(&str, &str); 2] = [
    ("exchanger", EXCHANGER_ADDRESS),
    ("jstz_native_bridge", JSTZ_NATIVE_BRIDGE_ADDRESS),
];
pub(crate) const BOOTSTRAP_ACCOUNTS: [(&str, &str, &str); 6] = [
    (
        "bootstrap0",
        "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
        "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
    ),
    (
        "bootstrap1",
        "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
    ),
    (
        "bootstrap2",
        "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
        "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
    ),
    (
        "bootstrap3",
        "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
        "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
    ),
    (
        "bootstrap4",
        "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
        "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
    ),
    (
        "bootstrap5",
        "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
        "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
    ),
];
pub const ROLLUP_OPERATOR_ACCOUNT_ALIAS: &str = "bootstrap1";
const BOOTSTRAP_ACCOUNT_BALANCE: u64 = 100_000_000_000;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/bootstrap_contract/"]
pub struct BootstrapContractFile;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/jstz_rollup"]
#[include = "*.json"]
struct BootstrapRollupFile;

#[derive(Deserialize, Default)]
pub struct Config {
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

pub(crate) async fn build_config_from_path(
    config_path: &Option<String>,
) -> Result<(u16, JstzdConfig)> {
    let config = match config_path {
        Some(p) => parse_config(p).await?,
        None => Config::default(),
    };
    build_config(config).await
}

pub async fn build_config(mut config: Config) -> Result<(u16, JstzdConfig)> {
    patch_octez_node_config(&mut config.octez_node);
    let octez_node_config = config.octez_node.build()?;
    let octez_client_config = match config.octez_client {
        Some(v) => v,
        None => OctezClientConfigBuilder::new(octez_node_config.rpc_endpoint.clone()),
    }
    .build()?;
    let protocol_params = build_protocol_params(config.protocol).await?;
    let baker_config = populate_baker_config(
        config.octez_baker,
        &octez_node_config,
        &octez_client_config,
        &protocol_params,
    )?;

    let octez_node_endpoint = octez_node_config.rpc_endpoint.clone();
    let kernel_debug_file = FileWrapper::default();
    let kernel_debug_file_path = kernel_debug_file.path();
    let octez_rollup_config = OctezRollupConfigBuilder::new(
        octez_node_endpoint,
        octez_client_config.base_dir().into(),
        SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        ROLLUP_OPERATOR_ACCOUNT_ALIAS.to_string(),
        jstz_rollup_path::kernel_installer_path(),
    )
    .set_data_dir(RollupDataDir::TempWithPreImages {
        preimages_dir: jstz_rollup_path::preimages_path(),
    })
    .set_kernel_debug_file(kernel_debug_file)
    .build()
    .unwrap();

    let jstz_node_rpc_endpoint =
        Endpoint::try_from(Uri::from_static(DEFAULT_JSTZ_NODE_ENDPOINT)).unwrap();
    let jstz_node_config = JstzNodeConfig::new(
        &jstz_node_rpc_endpoint,
        &octez_rollup_config.rpc_endpoint,
        &jstz_rollup_path::preimages_path(),
        &kernel_debug_file_path,
        KeyPair::default(),
    );

    let server_port = config.server_port.unwrap_or(DEFAULT_JSTZD_SERVER_PORT);
    Ok((
        server_port,
        JstzdConfig::new(
            octez_node_config,
            baker_config,
            octez_client_config,
            octez_rollup_config,
            jstz_node_config,
            protocol_params,
        ),
    ))
}

fn patch_octez_node_config(builder: &mut OctezNodeConfigBuilder) {
    let mut option_builder = OctezNodeRunOptionsBuilder::new();
    if let Some(v) = builder.run_options() {
        option_builder
            .set_network(v.network())
            .set_synchronisation_threshold(v.synchronisation_threshold());
        if let Some(mode) = v.history_mode() {
            option_builder.set_history_mode(mode.clone());
        }
    }
    if option_builder.history_mode().is_none() {
        option_builder.set_history_mode(OctezNodeHistoryMode::Rolling(15));
    }
    builder.set_run_options(&option_builder.build());
}

fn populate_baker_config(
    mut config_builder: OctezBakerConfigBuilder,
    octez_node_config: &OctezNodeConfig,
    octez_client_config: &OctezClientConfig,
    protocol_params: &ProtocolParameter,
) -> Result<OctezBakerConfig> {
    if config_builder.binary_path().is_none() {
        config_builder = config_builder
            .set_binary_path(BakerBinaryPath::Env(protocol_params.protocol()));
    }
    if config_builder.octez_client_base_dir().is_none() {
        config_builder = config_builder
            .set_octez_client_base_dir(&octez_client_config.base_dir().to_string());
    }
    if config_builder.octez_node_endpoint().is_none() {
        config_builder =
            config_builder.set_octez_node_endpoint(&octez_node_config.rpc_endpoint);
    }
    config_builder.build()
}

async fn read_bootstrap_contracts() -> Result<Vec<BootstrapContract>> {
    let mut contracts = vec![];
    for (contract_name, hash) in BOOTSTRAP_CONTRACT_NAMES {
        let script = serde_json::from_slice(
            &BootstrapContractFile::get(&format!("{contract_name}.json"))
                .ok_or(anyhow::anyhow!("file not found"))?
                .data,
        )
        .context(format!(
            "error loading bootstrap contract '{contract_name}'"
        ))?;
        contracts.push(BootstrapContract::new(script, 1_000_000, Some(hash)).unwrap());
    }
    Ok(contracts)
}

async fn build_protocol_params(
    mut builder: ProtocolParameterBuilder,
) -> Result<ProtocolParameter> {
    // User contracts whose addresses collide with those reserved for jstz contracts
    // will overwrite jstz contracts. This aligns with the current implementation
    // where bootstrap contracts in the base parameter file take precedence, even
    // if it means that jstz won't launch in such cases.
    let mut contracts = builder
        .bootstrap_contracts()
        .iter()
        .map(|v| (*v).to_owned())
        .collect::<Vec<BootstrapContract>>();
    for contract in read_bootstrap_contracts().await? {
        contracts.push(contract);
    }

    // Insert necessary bootstrap accounts. These accounts will be overwriten
    // when bootstrap accounts in users' parameter file collide with these bootstrap accounts.
    let mut accounts = builder
        .bootstrap_accounts()
        .iter()
        .map(|v| (*v).to_owned())
        .collect::<Vec<BootstrapAccount>>();
    for account in BOOTSTRAP_ACCOUNTS
        .map(|(_, pk, _)| BootstrapAccount::new(pk, BOOTSTRAP_ACCOUNT_BALANCE).unwrap())
    {
        accounts.push(account);
    }

    builder
        .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
            JSTZ_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Wasm,
            &tokio::fs::read_to_string(jstz_rollup_path::kernel_installer_path()).await?,
            serde_json::from_slice(
                &BootstrapRollupFile::get("parameters_ty.json")
                    .ok_or(anyhow::anyhow!("file not found"))?
                    .data,
            )?,
        )
        .unwrap()])
        .set_bootstrap_contracts(contracts)
        .set_bootstrap_accounts(accounts)
        .build()
}

#[cfg(test)]
mod tests {
    use std::{io::Read, io::Write, path::PathBuf, str::FromStr};

    use super::{jstz_rollup_path, Config, JSTZ_ROLLUP_ADDRESS};
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
        rollup::RollupDataDir,
    };
    use tempfile::{tempdir, NamedTempFile};
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tokio::io::AsyncReadExt;

    const ACCOUNT_PUBLIC_KEY: &str = super::BOOTSTRAP_ACCOUNTS[0].1;

    async fn read_param_file(path: &PathBuf) -> serde_json::Value {
        let mut buf = String::new();
        tokio::fs::File::open(path)
            .await
            .unwrap()
            .read_to_string(&mut buf)
            .await
            .unwrap();
        serde_json::from_str::<serde_json::Value>(&buf).unwrap()
    }

    async fn read_bootstrap_contracts_from_param_file(
        path: &PathBuf,
    ) -> Vec<BootstrapContract> {
        let params_json = read_param_file(path).await;
        params_json
            .as_object()
            .unwrap()
            .get("bootstrap_contracts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| serde_json::from_value::<BootstrapContract>(v.to_owned()).unwrap())
            .collect::<Vec<BootstrapContract>>()
    }

    async fn read_bootstrap_accounts_from_param_file(
        path: &PathBuf,
    ) -> Vec<BootstrapAccount> {
        let params_json = read_param_file(path).await;
        params_json
            .as_object()
            .unwrap()
            .get("bootstrap_accounts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| serde_json::from_value::<BootstrapAccount>(v.to_owned()).unwrap())
            .collect::<Vec<BootstrapAccount>>()
    }

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
                "protocol": "rio",
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
            .set_protocol(Protocol::Rio)
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

    #[test]
    fn populate_baker_config() {
        let log_file = NamedTempFile::new().unwrap().into_temp_path();
        let tmp_dir = tempdir().unwrap();
        let node_config = OctezNodeConfigBuilder::new()
            .set_rpc_endpoint(&Endpoint::localhost(5678))
            .build()
            .unwrap();
        let client_config = OctezClientConfigBuilder::new(Endpoint::localhost(5678))
            .set_base_dir(tmp_dir.path().to_path_buf())
            .build()
            .unwrap();
        let baker_builder = OctezBakerConfigBuilder::new().set_log_file(&log_file);
        let protocol_params = ProtocolParameterBuilder::new()
            .set_protocol(Protocol::Rio)
            .set_bootstrap_accounts([BootstrapAccount::new(
                ACCOUNT_PUBLIC_KEY,
                40_000_000_000,
            )
            .unwrap()])
            .build()
            .unwrap();
        let baker_config = super::populate_baker_config(
            baker_builder,
            &node_config,
            &client_config,
            &protocol_params,
        )
        .unwrap();

        // baker path is not provided in the config, so the builder takes the protocol version from
        // protocol_params
        assert_eq!(
            baker_config,
            OctezBakerConfigBuilder::new()
                .set_binary_path(BakerBinaryPath::Env(Protocol::Rio))
                .set_octez_client_base_dir(tmp_dir.path().to_str().unwrap())
                .set_octez_node_endpoint(&Endpoint::localhost(5678))
                .set_log_file(&log_file)
                .build()
                .unwrap()
        );

        // baker path is provided in the config, so the builder takes that path and ignores protocol_params
        let baker_builder = OctezBakerConfigBuilder::new()
            .set_binary_path(BakerBinaryPath::Custom(
                PathBuf::from_str("/foo/bar").unwrap(),
            ))
            .set_log_file(&log_file);
        let baker_config = super::populate_baker_config(
            baker_builder,
            &node_config,
            &client_config,
            &protocol_params,
        )
        .unwrap();
        assert_eq!(
            baker_config,
            OctezBakerConfigBuilder::new()
                .set_binary_path(BakerBinaryPath::Custom(
                    PathBuf::from_str("/foo/bar").unwrap()
                ))
                .set_octez_client_base_dir(tmp_dir.path().to_str().unwrap())
                .set_octez_node_endpoint(&Endpoint::localhost(5678))
                .set_log_file(&log_file)
                .build()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn build_config() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = serde_json::to_string(&serde_json::json!({
            "octez_node": {
                "rpc_endpoint": "localhost:8888",
            },
            "octez_client": {
                "octez_node_endpoint": "localhost:9999",
            },
            "protocol": {
                "bootstrap_accounts": [["edpktkhoky4f5kqm2EVwYrMBq5rY9sLYdpFgXixQDWifuBHjhuVuNN", "6000000000"]]
            }
        }))
        .unwrap();
        tmp_file.write_all(content.as_bytes()).unwrap();

        let (port, config) = super::build_config_from_path(&Some(
            tmp_file.path().to_str().unwrap().to_owned(),
        ))
        .await
        .unwrap();
        assert_eq!(
            config.octez_client_config().octez_node_endpoint(),
            &Endpoint::localhost(9999)
        );
        assert_eq!(port, super::DEFAULT_JSTZD_SERVER_PORT);

        let config_path = config
            .protocol_params()
            .parameter_file()
            .path()
            .to_path_buf();
        let contracts = read_bootstrap_contracts_from_param_file(&config_path).await;
        assert_eq!(contracts.len(), 2);

        let accounts = read_bootstrap_accounts_from_param_file(&config_path).await;
        assert_eq!(accounts.len(), 7);

        assert_eq!(
            config.octez_rollup_config().address.to_base58_check(),
            JSTZ_ROLLUP_ADDRESS
        );
        assert_eq!(
            config.octez_rollup_config().data_dir,
            RollupDataDir::TempWithPreImages {
                preimages_dir: jstz_rollup_path::preimages_path(),
            }
        );
        assert_eq!(
            config.octez_rollup_config().boot_sector_file,
            jstz_rollup_path::kernel_installer_path()
        );

        assert_eq!(
            config.jstz_node_config().rollup_endpoint,
            config.octez_rollup_config().rpc_endpoint
        );
    }

    #[tokio::test]
    async fn build_config_with_default_config() {
        let (_, config) = super::build_config_from_path(&None).await.unwrap();
        assert_eq!(
            config.octez_node_config().run_options.history_mode(),
            Some(&OctezNodeHistoryMode::Rolling(15))
        );

        let mut buf = String::new();
        config
            .protocol_params()
            .parameter_file()
            .read_to_string(&mut buf)
            .unwrap();
        let params = serde_json::from_str::<serde_json::Value>(&buf).unwrap();

        // two bootstrap account should have been inserted: the activator account and the rollup operator account
        let accounts = params
            .as_object()
            .unwrap()
            .get("bootstrap_accounts")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(accounts.len(), 6);

        let bootstrap_accounts = accounts
            .iter()
            .map(|acc| serde_json::from_value::<BootstrapAccount>(acc.clone()).unwrap())
            .collect::<Vec<_>>();

        for (_, pk, _) in super::BOOTSTRAP_ACCOUNTS {
            assert!(
                bootstrap_accounts.contains(
                    &BootstrapAccount::new(pk, super::BOOTSTRAP_ACCOUNT_BALANCE).unwrap()
                ),
                "account {pk} not found in bootstrap accounts"
            );
        }
    }

    #[tokio::test]
    async fn build_config_without_octez_client() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = serde_json::to_string(&serde_json::json!({
            "octez_node": {
                "rpc_endpoint": "localhost:8888",
            },
            "protocol": {
                "bootstrap_accounts": [["edpktkhoky4f5kqm2EVwYrMBq5rY9sLYdpFgXixQDWifuBHjhuVuNN", "6000000000"]]
            }
        }))
        .unwrap();
        tmp_file.write_all(content.as_bytes()).unwrap();
        let (_, config) = super::build_config_from_path(&Some(
            tmp_file.path().to_str().unwrap().to_owned(),
        ))
        .await
        .unwrap();
        assert_eq!(
            config.octez_client_config().octez_node_endpoint(),
            &Endpoint::localhost(8888)
        );
    }

    #[tokio::test]
    async fn read_bootstrap_contracts() {
        let mut contracts = super::read_bootstrap_contracts()
            .await
            .unwrap()
            .iter()
            .map(|v| v.hash().to_owned())
            .collect::<Vec<Option<ContractKt1Hash>>>();
        contracts.sort();
        assert_eq!(
            contracts,
            vec![
                Some(
                    ContractKt1Hash::from_base58_check(super::EXCHANGER_ADDRESS).unwrap()
                ),
                Some(
                    ContractKt1Hash::from_base58_check(super::JSTZ_NATIVE_BRIDGE_ADDRESS)
                        .unwrap()
                )
            ]
        )
    }

    #[tokio::test]
    async fn build_protocol_params() {
        let mut builder = ProtocolParameterBuilder::new();
        builder.set_bootstrap_accounts([BootstrapAccount::new(
            ACCOUNT_PUBLIC_KEY,
            40_000_000_000,
        )
        .unwrap()]);
        let params = super::build_protocol_params(builder).await.unwrap();
        let mut addresses = read_bootstrap_contracts_from_param_file(
            &params.parameter_file().path().to_path_buf(),
        )
        .await
        .iter()
        .map(|v| v.hash().as_ref().unwrap().clone().to_string())
        .collect::<Vec<String>>();
        addresses.sort();
        assert_eq!(
            addresses,
            [super::EXCHANGER_ADDRESS, super::JSTZ_NATIVE_BRIDGE_ADDRESS]
        );
    }

    #[tokio::test]
    async fn build_protocol_params_contract_collision() {
        let dummy_contract = BootstrapContract::new(
            serde_json::json!("test-contract"),
            1,
            Some(super::EXCHANGER_ADDRESS),
        )
        .unwrap();
        let mut builder = ProtocolParameterBuilder::new();
        builder
            .set_bootstrap_accounts([BootstrapAccount::new(
                ACCOUNT_PUBLIC_KEY,
                40_000_000_000,
            )
            .unwrap()])
            .set_bootstrap_contracts([dummy_contract.clone()]);
        let params = super::build_protocol_params(builder).await.unwrap();
        let mut contracts = read_bootstrap_contracts_from_param_file(
            &params.parameter_file().path().to_path_buf(),
        )
        .await;
        assert_eq!(contracts.len(), 2);

        contracts.sort_by_key(|v| v.hash().as_ref().unwrap().to_string());
        let addresses = contracts
            .iter()
            .map(|v| v.hash().to_owned().unwrap().to_string())
            .collect::<Vec<String>>();
        assert_eq!(
            addresses,
            [super::EXCHANGER_ADDRESS, super::JSTZ_NATIVE_BRIDGE_ADDRESS]
        );
        // the first contract should be overwritten by the dummy contract
        let exchanger_contract = contracts.first().unwrap();
        assert_eq!(exchanger_contract, &dummy_contract);
    }

    #[test]
    fn patch_octez_node_config() {
        let mut builder = OctezNodeConfigBuilder::default();
        super::patch_octez_node_config(&mut builder);
        assert_eq!(
            builder.run_options().unwrap().history_mode(),
            Some(&OctezNodeHistoryMode::Rolling(15))
        );

        // should fill in history mode but not overwrite existing run options
        let mut builder = OctezNodeConfigBuilder::default();
        builder.set_run_options(
            &OctezNodeRunOptionsBuilder::new()
                .set_network("test")
                .set_synchronisation_threshold(3)
                .build(),
        );
        super::patch_octez_node_config(&mut builder);
        let run_options = builder.run_options().unwrap();
        assert_eq!(
            run_options.history_mode(),
            Some(&OctezNodeHistoryMode::Rolling(15))
        );
        assert_eq!(run_options.network(), "test");
        assert_eq!(run_options.synchronisation_threshold(), 3);

        // should not overwrite existing run options
        let mut builder = OctezNodeConfigBuilder::default();
        let run_options = OctezNodeRunOptionsBuilder::new()
            .set_network("test")
            .set_synchronisation_threshold(3)
            .set_history_mode(OctezNodeHistoryMode::Archive)
            .build();
        builder.set_run_options(&run_options);
        super::patch_octez_node_config(&mut builder);
        assert_eq!(builder.run_options().unwrap(), &run_options);
    }
}
