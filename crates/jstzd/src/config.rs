use std::io::Write;
use std::path::PathBuf;

use jstz_crypto::public_key::PublicKey;
use jstz_crypto::secret_key::SecretKey;
#[cfg(feature = "oracle")]
use jstz_oracle_node::OracleNodeConfig;
use jstz_utils::KeyPair;
use octez::r#async::node_config::{OctezNodeHistoryMode, OctezNodeRunOptionsBuilder};
use rust_embed::Embed;
use tempfile::NamedTempFile;

use crate::task::jstzd::JstzdConfig;
use crate::{
    jstz_rollup_path, EXCHANGER_ADDRESS, JSTZ_NATIVE_BRIDGE_ADDRESS, JSTZ_ROLLUP_ADDRESS,
};
use anyhow::{Context, Result};
use http::Uri;
use jstz_node::config::{JstzNodeConfig, RunModeBuilder, RunModeType};
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
pub(crate) const ROLLUP_OPERATOR_ACCOUNT_ALIAS: &str = "rollup_operator";
pub(crate) const ACTIVATOR_ACCOUNT_ALIAS: &str = "activator";
pub(crate) const INJECTOR_ACCOUNT_ALIAS: &str = "injector";

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/bootstrap_contract/"]
pub struct BootstrapContractFile;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/bootstrap_account/"]
pub struct BootstrapAccountFile;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/jstz_rollup"]
#[include = "*.json"]
struct BootstrapRollupFile;

// A subset of JstzNodeConfig that is exposed to users.
#[derive(Deserialize, Default, PartialEq, Debug, Clone)]
struct UserJstzNodeConfig {
    mode: Option<RunModeType>,
    capacity: Option<usize>,
    debug_log_file: Option<PathBuf>,
    riscv_kernel_path: Option<PathBuf>,
    rollup_address: Option<SmartRollupHash>,
}

#[derive(Deserialize, Default)]
pub struct Config {
    server_port: Option<u16>,
    #[serde(default)]
    octez_node: OctezNodeConfigBuilder,
    #[serde(default)]
    octez_baker: OctezBakerConfigBuilder,
    octez_client: Option<OctezClientConfigBuilder>,
    #[serde(default)]
    octez_rollup: Option<OctezRollupConfigBuilder>,
    #[serde(default)]
    jstz_node: UserJstzNodeConfig,
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

pub(crate) fn builtin_bootstrap_accounts() -> Result<Vec<(String, String, String, u64)>> {
    let accounts = serde_json::from_slice(
        &BootstrapAccountFile::get("accounts.json")
            .ok_or(anyhow::anyhow!("bootstrap account file not found"))?
            .data,
    )
    .context("error loading built-in bootstrap accounts")?;
    validate_builtin_bootstrap_accounts(accounts)
}

// This is split from `builtin_bootstrap_accounts` just to make the logic easily testable
fn validate_builtin_bootstrap_accounts(
    accounts: Vec<(String, String, String, u64)>,
) -> Result<Vec<(String, String, String, u64)>> {
    if accounts.iter().fold(0, |acc, (alias, _, _, _)| {
        acc + (alias == ACTIVATOR_ACCOUNT_ALIAS) as usize
    }) != 1
    {
        anyhow::bail!(
            "there must be exactly one built-in bootstrap account with alias '{ACTIVATOR_ACCOUNT_ALIAS}'"
        )
    }
    Ok(accounts)
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
    patch_octez_node_config(&mut config.octez_node)
        .context("failed to patch octez node config")?;
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

    let mut rollup_builder = config.octez_rollup.unwrap_or_default();

    if !rollup_builder.has_octez_client_base_dir() {
        rollup_builder = rollup_builder
            .set_octez_client_base_dir(octez_client_config.base_dir().into());
    }
    if !rollup_builder.has_octez_node_endpoint() {
        rollup_builder = rollup_builder.set_octez_node_endpoint(&octez_node_endpoint);
    }
    if !rollup_builder.has_address() {
        rollup_builder = rollup_builder.set_address(
            SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        );
    }
    if !rollup_builder.has_operator() {
        rollup_builder =
            rollup_builder.set_operator(ROLLUP_OPERATOR_ACCOUNT_ALIAS.to_string());
    }
    if !rollup_builder.has_boot_sector_file() {
        rollup_builder = rollup_builder
            .set_boot_sector_file(jstz_rollup_path::kernel_installer_path());
    }

    let octez_rollup_config = rollup_builder
        .set_data_dir(RollupDataDir::TempWithPreImages {
            preimages_dir: jstz_rollup_path::preimages_path(),
        })
        .set_kernel_debug_file(kernel_debug_file)
        .build()
        .unwrap();

    let jstz_node_config = build_jstz_node_config(
        config.jstz_node,
        &octez_rollup_config.rpc_endpoint,
        &kernel_debug_file_path,
    )
    .context("failed to build jstz node config")?;

    let server_port = config.server_port.unwrap_or(DEFAULT_JSTZD_SERVER_PORT);
    Ok((
        server_port,
        JstzdConfig::new(
            octez_node_config,
            baker_config,
            octez_client_config,
            octez_rollup_config,
            #[cfg(feature = "oracle")]
            build_oracle_config(Some(injector.clone()), &jstz_node_config),
            jstz_node_config,
            protocol_params,
        ),
    ))
}

fn build_jstz_node_config(
    config: UserJstzNodeConfig,
    rollup_rpc_endpoint: &Endpoint,
    kernel_debug_file_path: &PathBuf,
) -> Result<JstzNodeConfig> {
    let jstz_node_rpc_endpoint =
        Endpoint::try_from(Uri::from_static(DEFAULT_JSTZ_NODE_ENDPOINT)).unwrap();
    let injector = find_injector_account(builtin_bootstrap_accounts()?)
        .context("failed to retrieve injector account")?;
    let mut run_mode_builder = RunModeBuilder::new(config.mode.unwrap_or_default());
    if let Some(v) = config.capacity {
        run_mode_builder = run_mode_builder.with_capacity(v)?;
    }
    if let Some(path) = config.debug_log_file {
        run_mode_builder = run_mode_builder.with_debug_log_path(path)?;
    }
    if let Some(path) = config.riscv_kernel_path {
        run_mode_builder = run_mode_builder.with_riscv_kernel_path(path)?;
    }
    if let Some(v) = config.rollup_address {
        run_mode_builder = run_mode_builder.with_rollup_address(v)?;
    }
    Ok(JstzNodeConfig::new(
        &jstz_node_rpc_endpoint,
        rollup_rpc_endpoint,
        &jstz_rollup_path::preimages_path(),
        kernel_debug_file_path,
        injector.clone(),
        run_mode_builder.build()?,
    ))
}

fn patch_octez_node_config(builder: &mut OctezNodeConfigBuilder) -> Result<()> {
    let config_path = create_sandbox_config_file(builtin_bootstrap_accounts()?)
        .context("failed to create sandbox config file")?;
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
    option_builder.set_sandbox_config_path(&config_path);
    builder.set_run_options(&option_builder.build());
    Ok(())
}

fn find_injector_account(
    bootstrap_accounts: Vec<(String, String, String, u64)>,
) -> Result<KeyPair> {
    for (alias, pk, raw_sk, _) in bootstrap_accounts {
        if alias == INJECTOR_ACCOUNT_ALIAS {
            let sk = if raw_sk.starts_with("unencrypted") {
                raw_sk.split(':').nth(1).unwrap()
            } else {
                &raw_sk
            };
            return Ok(KeyPair(
                PublicKey::from_base58(&pk)?,
                SecretKey::from_base58(sk)?,
            ));
        }
    }
    anyhow::bail!("cannot find injector account")
}

#[cfg(feature = "oracle")]
fn build_oracle_config(
    key_pair: Option<KeyPair>,
    jstz_node_config: &JstzNodeConfig,
) -> OracleNodeConfig {
    OracleNodeConfig {
        key_pair,
        jstz_node_endpoint: jstz_node_config.endpoint.clone(),
        log_path: match &jstz_node_config.mode {
            RunMode::Default => jstz_node_config.kernel_log_file.clone(),
            RunMode::Sequencer { debug_log_path, .. } => debug_log_path.clone(),
        },
    }
}

// Create a sandbox config file that informs octez node about the activator account.
// Normally this is not necessary as octez node has a hard-coded default activator account,
// but if we want to use a different account, we need to specify it in this config file.
fn create_sandbox_config_file(
    bootstrap_accounts: Vec<(String, String, String, u64)>,
) -> Result<PathBuf> {
    for (alias, pk, _, _) in bootstrap_accounts {
        if alias == ACTIVATOR_ACCOUNT_ALIAS {
            let (mut config_file, p) = NamedTempFile::new()?.keep()?;
            config_file
                .write_all(
                    &serde_json::to_vec(&serde_json::json!({
                      "genesis_pubkey": pk
                    }))
                    .context("failed to serialise sandbox config")?,
                )
                .context("failed to write to sandbox config file")?;
            config_file
                .flush()
                .context("failed to flush sandbox config file")?;
            return Ok(p);
        }
    }
    anyhow::bail!("cannot find activator account")
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
    for account in
        builtin_bootstrap_accounts()?
            .into_iter()
            .map(|(_, pk, _, balance_mutez)| {
                BootstrapAccount::new(&pk, balance_mutez).unwrap()
            })
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

    use crate::config::UserJstzNodeConfig;

    use super::{jstz_rollup_path, Config, JSTZ_ROLLUP_ADDRESS};
    use http::Uri;
    use jstz_node::{config::RuntimeEnv, RunMode};
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
        rollup::{HistoryMode, RollupDataDir},
    };
    use tempfile::{tempdir, NamedTempFile};
    use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
    use tokio::io::AsyncReadExt;

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
    fn user_jstz_node_config() {
        assert_eq!(
            UserJstzNodeConfig::default(),
            UserJstzNodeConfig {
                mode: None,
                capacity: None,
                debug_log_file: None,
                riscv_kernel_path: None,
                rollup_address: None
            }
        )
    }

    #[test]
    fn deserialize_config_default() {
        let config = serde_json::from_value::<Config>(serde_json::json!({})).unwrap();
        assert_eq!(config.octez_baker, OctezBakerConfigBuilder::default());
        assert!(config.octez_client.is_none());
        assert_eq!(config.octez_node, OctezNodeConfigBuilder::default());
        assert_eq!(config.protocol, ProtocolParameterBuilder::default());
        assert!(config.server_port.is_none());
        assert_eq!(config.jstz_node, UserJstzNodeConfig::default());
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
    fn deserialize_config_jstz_node() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "jstz_node": {
                "mode": "sequencer",
                "capacity": 42,
                "debug_log_file": "/tmp/log",
                "riscv_kernel_path": "/riscv/kernel",
                "rollup_address": "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK"
            }
        }))
        .unwrap();
        assert_eq!(
            config.jstz_node,
            UserJstzNodeConfig {
                mode: Some(jstz_node::config::RunModeType::Sequencer),
                capacity: Some(42),
                debug_log_file: Some(PathBuf::from_str("/tmp/log").unwrap()),
                riscv_kernel_path: Some(PathBuf::from_str("/riscv/kernel").unwrap()),
                rollup_address: Some(
                    SmartRollupHash::from_base58_check(
                        "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK"
                    )
                    .unwrap()
                )
            }
        );

        // default
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "jstz_node": {}
        }))
        .unwrap();
        assert_eq!(
            config.jstz_node,
            UserJstzNodeConfig {
                mode: None,
                capacity: None,
                debug_log_file: None,
                riscv_kernel_path: None,
                rollup_address: None
            }
        );
    }

    #[test]
    fn deserialize_config_partial_rollup() {
        let config = serde_json::from_value::<Config>(serde_json::json!({
            "octez_rollup": {
                "rpc_endpoint": "http://0.0.0.0:18741"
            }
        }))
        .unwrap();

        assert!(config.octez_rollup.is_some());
        let builder = config.octez_rollup.unwrap();
        assert!(builder.rpc_endpoint.is_some());
        assert!(!builder.has_octez_client_base_dir());
        assert!(!builder.has_octez_node_endpoint());
        assert!(!builder.has_address());
        assert!(!builder.has_operator());
        assert!(!builder.has_boot_sector_file());
    }

    #[tokio::test]
    async fn build_config_with_partial_rollup() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = serde_json::to_string(&serde_json::json!({
            "octez_rollup": {
                "rpc_endpoint": "http://0.0.0.0:18741",
                "history_mode": "archive"
            }
        }))
        .unwrap();
        tmp_file.write_all(content.as_bytes()).unwrap();

        let (port, config) = super::build_config_from_path(&Some(
            tmp_file.path().to_str().unwrap().to_owned(),
        ))
        .await
        .unwrap();

        // Should successfully build with defaults
        assert_eq!(port, super::DEFAULT_JSTZD_SERVER_PORT);

        // Check that the RPC endpoint was preserved
        assert_eq!(
            config.octez_rollup_config().rpc_endpoint,
            Endpoint::try_from(Uri::from_str("http://0.0.0.0:18741").unwrap()).unwrap()
        );

        // Check that defaults were applied
        assert_eq!(
            config.octez_rollup_config().address.to_base58_check(),
            JSTZ_ROLLUP_ADDRESS
        );
        assert_eq!(
            config.octez_rollup_config().operator,
            super::ROLLUP_OPERATOR_ACCOUNT_ALIAS
        );
        assert_eq!(
            config.octez_rollup_config().history_mode,
            HistoryMode::Archive
        );
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
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
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
            },
            "jstz_node": {
                "mode": "sequencer",
                "capacity": 42,
                "debug_log_file": "/debug/file"
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
        assert_eq!(accounts.len(), 9);

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
        assert_eq!(
            config.jstz_node_config().mode,
            RunMode::Sequencer {
                capacity: 42,
                debug_log_path: PathBuf::from_str("/debug/file").unwrap(),
                runtime_env: RuntimeEnv::Native,
            }
        );
    }

    #[test]
    fn build_jstz_node_config() {
        let rollup_address =
            SmartRollupHash::from_base58_check("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK")
                .unwrap();
        let config = UserJstzNodeConfig {
            mode: Some(jstz_node::config::RunModeType::Sequencer),
            capacity: Some(42),
            debug_log_file: Some(PathBuf::from_str("/tmp/log").unwrap()),
            riscv_kernel_path: Some(PathBuf::from_str("/riscv/kernel").unwrap()),
            rollup_address: Some(rollup_address.clone()),
        };
        let jstz_node_config =
            super::build_jstz_node_config(config, &Endpoint::default(), &PathBuf::new())
                .unwrap();
        assert_eq!(
            jstz_node_config.mode,
            RunMode::Sequencer {
                capacity: 42,
                debug_log_path: PathBuf::from_str("/tmp/log").unwrap(),
                runtime_env: RuntimeEnv::Riscv {
                    kernel_path: PathBuf::from_str("/riscv/kernel").unwrap(),
                    rollup_address: rollup_address
                },
            }
        );

        let bad_config = UserJstzNodeConfig {
            riscv_kernel_path: Some(PathBuf::new()),
            ..Default::default()
        };
        assert!(super::build_jstz_node_config(
            bad_config,
            &Endpoint::default(),
            &PathBuf::new(),
        )
        .is_err());
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

        // built-in bootstrap accounts
        let accounts = params
            .as_object()
            .unwrap()
            .get("bootstrap_accounts")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(accounts.len(), 8);

        let bootstrap_accounts = accounts
            .iter()
            .map(|acc| serde_json::from_value::<BootstrapAccount>(acc.clone()).unwrap())
            .collect::<Vec<_>>();

        for (_, pk, _, balance_mutez) in super::builtin_bootstrap_accounts().unwrap() {
            assert!(
                bootstrap_accounts
                    .contains(&BootstrapAccount::new(&pk, balance_mutez).unwrap()),
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
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
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
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
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
        super::patch_octez_node_config(&mut builder).unwrap();
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
        super::patch_octez_node_config(&mut builder).unwrap();
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
        super::patch_octez_node_config(&mut builder).unwrap();
        let stored_run_options = builder.run_options().unwrap();
        assert_eq!(
            stored_run_options.history_mode(),
            Some(&OctezNodeHistoryMode::Archive)
        );
        assert_eq!(stored_run_options.synchronisation_threshold(), 3);
        assert_eq!(stored_run_options.network(), "test");
        let sandbox_config_path = stored_run_options.sandbox_config_path().unwrap();
        let content = serde_json::from_reader::<_, serde_json::Value>(
            std::fs::File::open(sandbox_config_path).unwrap(),
        )
        .unwrap();
        assert_eq!(
            content,
            serde_json::json!({
                // should be the activator in resources/bootstrap_account/accounts.json
                "genesis_pubkey": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
            })
        );
    }

    #[test]
    fn create_sandbox_config_file() {
        let err = super::create_sandbox_config_file(vec![]).unwrap_err();
        assert_eq!(err.to_string(), "cannot find activator account");

        let path = super::create_sandbox_config_file(vec![(
            "activator".into(),
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav".into(),
            "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh".into(),
            1,
        )])
        .unwrap();
        let content = serde_json::from_reader::<_, serde_json::Value>(
            std::fs::File::open(path).unwrap(),
        )
        .unwrap();
        assert_eq!(
            content,
            serde_json::json!({
                "genesis_pubkey": "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav"
            })
        );
    }

    #[test]
    fn validate_builtin_bootstrap_accounts() {
        let activator = (
            "activator".into(),
            "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2".into(),
            "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6".into(),
            1,
        );
        let bootstrap1 = (
            "bootstrap1".into(),
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav".into(),
            "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh".into(),
            1,
        );

        let result = super::validate_builtin_bootstrap_accounts(vec![bootstrap1.clone()]);
        assert_eq!(
            result.unwrap_err().to_string(),
            "there must be exactly one built-in bootstrap account with alias 'activator'"
        );

        let result = super::validate_builtin_bootstrap_accounts(vec![
            activator.clone(),
            activator.clone(),
        ]);
        assert_eq!(
            result.unwrap_err().to_string(),
            "there must be exactly one built-in bootstrap account with alias 'activator'"
        );

        let result = super::validate_builtin_bootstrap_accounts(vec![
            activator.clone(),
            bootstrap1.clone(),
        ]);
        assert_eq!(result.unwrap(), vec![activator, bootstrap1]);
    }

    #[test]
    fn find_injector_account() {
        let injector = (
            "injector".into(),
            "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2".into(),
            "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6".into(),
            1,
        );
        let bootstrap1 = (
            "bootstrap1".into(),
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav".into(),
            "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh".into(),
            1,
        );

        let error = super::find_injector_account(vec![bootstrap1.clone()]).unwrap_err();
        assert_eq!(error.to_string(), "cannot find injector account");

        let keys = super::find_injector_account(vec![bootstrap1, injector]).unwrap();
        assert_eq!(
            keys.0.to_base58(),
            "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
        );
        assert_eq!(
            keys.1.to_base58(),
            "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
        );
    }

    #[cfg(feature = "oracle")]
    #[test]
    fn build_oracle_config() {
        let keys = jstz_utils::KeyPair(
            jstz_crypto::public_key::PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap(),
            jstz_crypto::secret_key::SecretKey::from_base58(
                "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            )
            .unwrap(),
        );
        let config = super::build_oracle_config(
            Some(keys.clone()),
            &jstz_node::config::JstzNodeConfig::new(
                &Endpoint::default(),
                &Endpoint::default(),
                &PathBuf::from("/foo/bar"),
                &PathBuf::from("/kernel/debug"),
                keys.clone(),
                jstz_node::RunMode::Default,
            ),
        );
        assert_eq!(config.log_path.to_str().unwrap(), "/kernel/debug");

        let config = super::build_oracle_config(
            Some(keys.clone()),
            &jstz_node::config::JstzNodeConfig::new(
                &Endpoint::default(),
                &Endpoint::default(),
                &PathBuf::from("/foo/bar"),
                &PathBuf::from("/kernel/debug"),
                keys.clone(),
                jstz_node::RunMode::Sequencer {
                    capacity: 0,
                    debug_log_path: PathBuf::from("/jstz_node/debug"),
                },
            ),
        );
        assert_eq!(config.log_path.to_str().unwrap(), "/jstz_node/debug");
    }
}
