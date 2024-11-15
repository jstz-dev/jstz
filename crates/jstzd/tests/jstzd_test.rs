mod utils;
use std::io::Read;
use std::str::FromStr;

use anyhow::Result;
use http::Uri;
use jstz_node::config::JstzNodeConfig;
use jstzd::task::jstzd::JstzdConfig;
use jstzd::task::utils::{get_block_level, retry};
use jstzd::{EXCHANGER_ADDRESS, JSTZ_NATIVE_BRIDGE_ADDRESS, JSTZ_ROLLUP_ADDRESS};
use octez::r#async::baker::{BakerBinaryPath, OctezBakerConfigBuilder};
use octez::r#async::client::{OctezClient, OctezClientConfig, OctezClientConfigBuilder};
use octez::r#async::endpoint::Endpoint;
use octez::r#async::node_config::{OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder};
use octez::r#async::protocol::{
    BootstrapAccount, BootstrapContract, BootstrapSmartRollup, Protocol,
    ProtocolParameter, ProtocolParameterBuilder, SmartRollupPvmKind,
};
use octez::r#async::rollup::{OctezRollupConfigBuilder, RollupDataDir};
use octez::unused_port;
use serde::Deserialize;
use serde_json::from_str;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tezos_crypto_rs::hash::SmartRollupHash;

const ACTIVATOR_PK: &str = "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2";
const CONTRACT_INIT_BALANCE: f64 = 1.0;
const CONTRACT_NAMES: [(&str, &str); 2] = [
    ("exchanger", EXCHANGER_ADDRESS),
    ("jstz_native_bridge", JSTZ_NATIVE_BRIDGE_ADDRESS),
];
pub const JSTZ_ROLLUP_OPERATOR_PK: &str =
    "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav";
pub const JSTZ_ROLLUP_OPERATOR_ALIAS: &str = "bootstrap1";

#[tokio::test(flavor = "multi_thread")]
async fn jstzd_test() {
    let node_rpc_endpoint = Endpoint::localhost(unused_port());
    let rollup_rpc_endpoint = Endpoint::try_from(
        Uri::from_str(&format!("http://127.0.0.1:{}", unused_port())).unwrap(),
    )
    .unwrap();
    let jstz_node_rpc_endpoint = Endpoint::localhost(unused_port());
    let jstzd_port = unused_port();
    let (jstzd_config, octez_client_config, kernel_debug_file) = build_configs(
        &node_rpc_endpoint,
        &rollup_rpc_endpoint,
        &jstz_node_rpc_endpoint,
    )
    .await;
    let mut jstzd = jstzd::task::jstzd::JstzdServer::new(jstzd_config, jstzd_port);
    // start jstzd
    jstzd.run().await.unwrap();

    let jstzd_health_check_endpoint = format!("http://localhost:{}/health", jstzd_port);
    let octez_node_health_check_endpoint = format!("{}/health/ready", node_rpc_endpoint);
    let jstzd_running = retry(30, 1000, || async {
        let res = reqwest::get(&jstzd_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(jstzd_running);

    // check if octez_node is running
    let node_running = retry(30, 1000, || async {
        let res = reqwest::get(&octez_node_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(node_running);

    // check if baker is running
    let baker_running = retry(30, 1000, || async {
        if run_ps().await.contains("octez-baker") {
            let last_level = get_block_level(&node_rpc_endpoint.to_string()).await?;
            return Ok(last_level > 2);
        }
        Ok(false)
    })
    .await;
    assert!(baker_running);

    // check bootstrap contracts
    let octez_client = OctezClient::new(octez_client_config);
    check_bootstrap_contracts(&octez_client).await;

    let rollup_running = retry(30, 1000, || async {
        let res = rollup_health_check(&rollup_rpc_endpoint.clone()).await;
        Ok(res.is_ok_and(|healthy| healthy))
    })
    .await;
    assert!(rollup_running);

    let jstz_node_is_running = retry(10, 1000, || async {
        let res = jstz_node_health_check(&jstz_node_rpc_endpoint.clone()).await;
        Ok(res.is_ok_and(|healthy| healthy))
    })
    .await;
    assert!(jstz_node_is_running);

    assert!(jstzd.health_check().await);

    // check if any logs are written to the kernel_debug_file
    let kernel_debug_content = read_file(kernel_debug_file).await;
    assert!(kernel_debug_content.contains("Internal message: start of level"));
    assert!(kernel_debug_content.contains("Internal message: end of level"));

    // stop jstzd
    jstzd.stop().await.unwrap();
    let jstzd_stopped = retry(30, 1000, || async {
        let res = reqwest::get(&jstzd_health_check_endpoint).await;
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Ok(false)
    })
    .await;
    assert!(jstzd_stopped);

    // check if the node, baker, rollup and jstz_node are destroyed
    let node_destroyed = retry(30, 1000, || async {
        let res = reqwest::get(&octez_node_health_check_endpoint).await;
        // Should get an error since the node should have been terminated
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Ok(false)
    })
    .await;
    assert!(node_destroyed);
    let baker_destroyed = retry(30, 1000, || async {
        Ok(!run_ps().await.contains("octez-baker"))
    })
    .await;
    assert!(baker_destroyed);
    let rollup_health = rollup_health_check(&rollup_rpc_endpoint.clone()).await;
    assert!(rollup_health.is_err());
    assert!(!jstzd.health_check().await);
}

async fn run_ps() -> String {
    let output = tokio::process::Command::new("ps")
        // print with extra columns so that commands don't get truncated too much
        .args(["-o", "args"])
        .env("COLUMNS", "1000")
        .output()
        .await
        .unwrap();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "");
    String::from_utf8(output.stdout).unwrap()
}

async fn build_configs(
    octez_node_rpc_endpoint: &Endpoint,
    rollup_rpc_endpoint: &Endpoint,
    jstz_node_rpc_endpoint: &Endpoint,
) -> (JstzdConfig, OctezClientConfig, NamedTempFile) {
    let run_options = OctezNodeRunOptionsBuilder::new()
        .set_synchronisation_threshold(0)
        .set_network("sandbox")
        .build();
    let octez_node_config = OctezNodeConfigBuilder::new()
        .set_network("sandbox")
        .set_rpc_endpoint(octez_node_rpc_endpoint)
        .set_run_options(&run_options)
        .build()
        .unwrap();

    let (rollup_kernel_installer, rollup_preimages_dir, rollup_parameters_ty) =
        jstz_rollup_files();

    let protocol_params: ProtocolParameter =
        build_protocol_params(&rollup_kernel_installer, &rollup_parameters_ty).await;

    let octez_client_config =
        OctezClientConfigBuilder::new(octez_node_rpc_endpoint.clone())
            .build()
            .unwrap();

    let baker_config = OctezBakerConfigBuilder::new()
        .set_binary_path(BakerBinaryPath::Env(protocol_params.protocol().clone()))
        .set_octez_client_base_dir(
            PathBuf::from(octez_client_config.base_dir())
                .to_str()
                .unwrap(),
        )
        .set_octez_node_endpoint(octez_node_rpc_endpoint)
        .build()
        .expect("Failed to build baker config");

    let kernel_debug_file = NamedTempFile::new().unwrap();
    let rollup_config = OctezRollupConfigBuilder::new(
        octez_node_config.rpc_endpoint.clone(),
        octez_client_config.base_dir().into(),
        SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        JSTZ_ROLLUP_OPERATOR_ALIAS.to_string(),
        rollup_kernel_installer,
    )
    .set_data_dir(RollupDataDir::TempWithPreImages {
        preimages_dir: rollup_preimages_dir,
    })
    .set_rpc_endpoint(rollup_rpc_endpoint)
    .set_kernel_debug_file(kernel_debug_file.path())
    .build()
    .expect("failed to build rollup config");

    let jstz_node_config = JstzNodeConfig::new(
        jstz_node_rpc_endpoint,
        &rollup_config.rpc_endpoint,
        kernel_debug_file.path(),
    );

    let jstzd_config = jstzd::task::jstzd::JstzdConfig::new(
        octez_node_config,
        baker_config,
        octez_client_config.clone(),
        rollup_config,
        jstz_node_config,
        protocol_params,
    );

    (jstzd_config, octez_client_config, kernel_debug_file)
}

fn jstz_rollup_files() -> (PathBuf, PathBuf, PathBuf) {
    let rollup_kernel_installer = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join("resources/jstz_rollup/kernel_installer");
    let rollup_preimages_dir = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join("resources/jstz_rollup/preimages");
    let rollup_parameters_ty = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join("resources/jstz_rollup/parameters_ty.json");

    (
        rollup_kernel_installer,
        rollup_preimages_dir,
        rollup_parameters_ty,
    )
}

async fn build_protocol_params(
    rollup_kernel_installer: &Path,
    rollup_parameters_ty: &Path,
) -> ProtocolParameter {
    ProtocolParameterBuilder::new()
        .set_protocol(Protocol::Alpha)
        .set_bootstrap_accounts([
            // activator is given at least 12000 (6000 for bootstrap + 6000 for baking) tez for baking
            BootstrapAccount::new(ACTIVATOR_PK, 15_000_000_000).unwrap(),
            BootstrapAccount::new(JSTZ_ROLLUP_OPERATOR_PK, 60_000_000_000).unwrap(),
        ])
        .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
            JSTZ_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Wasm,
            fs::read_to_string(rollup_kernel_installer)
                .unwrap()
                .as_str(),
            from_str(fs::read_to_string(rollup_parameters_ty).unwrap().as_str())
                .expect("failed to stringify JSON"),
        )
        .unwrap()])
        .set_bootstrap_contracts(read_bootstrap_contracts().await)
        .build()
        .expect("failed to build protocol parameters")
}

async fn read_bootstrap_contracts() -> Vec<BootstrapContract> {
    let mut contracts = vec![];
    for (contract_name, hash) in CONTRACT_NAMES {
        let script = utils::read_json_file(
            PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
                .join(format!("resources/bootstrap_contract/{contract_name}.json")),
        )
        .await;
        contracts.push(
            BootstrapContract::new(
                script,
                CONTRACT_INIT_BALANCE as u64 * 1_000_000,
                Some(hash),
            )
            .unwrap(),
        );
    }
    contracts
}

async fn check_bootstrap_contracts(octez_client: &OctezClient) {
    for (contract_name, hash) in CONTRACT_NAMES {
        assert_eq!(
            octez_client
                .get_balance(EXCHANGER_ADDRESS)
                .await
                .unwrap_or_else(|_| panic!(
                    "should be able to find contract '{contract_name}' at '{hash}'"
                )),
            CONTRACT_INIT_BALANCE,
            "balance mismatch for contract '{}'",
            contract_name
        );
    }
}

#[derive(Debug, Deserialize)]
struct HealthCheckResponse {
    healthy: bool,
}
pub async fn rollup_health_check(rollup_rpc_endpoint: &Endpoint) -> Result<bool> {
    let res = reqwest::get(format!("{}/health", rollup_rpc_endpoint)).await?;
    let body = res.json::<HealthCheckResponse>().await?;
    Ok(body.healthy)
}

async fn read_file(file: NamedTempFile) -> String {
    let mut file = file.reopen().unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    contents
}

pub async fn jstz_node_health_check(jstz_node_rpc_endpoint: &Endpoint) -> Result<bool> {
    let res = reqwest::get(format!("{}/health", jstz_node_rpc_endpoint)).await?;
    Ok(res.status().is_success())
}
