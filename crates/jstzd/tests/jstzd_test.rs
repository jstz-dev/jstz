mod utils;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[cfg(feature = "oracle")]
use jstz_crypto::keypair_from_mnemonic;
use jstz_crypto::public_key::PublicKey;
use jstz_crypto::secret_key::SecretKey;
use jstz_node::config::JstzNodeConfig;
#[cfg(feature = "oracle")]
use jstz_oracle_node::OracleNodeConfig;
use jstz_utils::KeyPair;
use jstzd::jstz_rollup_path::*;

use http::Uri;
use jstzd::task::jstzd::{JstzdConfig, JstzdServer};
use jstzd::task::utils::retry;
use jstzd::{BOOTSTRAP_CONTRACT_NAMES, JSTZ_ROLLUP_ADDRESS};
use octez::r#async::baker::{BakerBinaryPath, OctezBakerConfigBuilder};
use octez::r#async::client::{OctezClient, OctezClientConfigBuilder};
use octez::r#async::endpoint::Endpoint;
use octez::r#async::file::FileWrapper;
use octez::r#async::node_config::{OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder};
use octez::r#async::protocol::{
    BootstrapAccount, BootstrapContract, BootstrapSmartRollup, ProtocolParameterBuilder,
    SmartRollupPvmKind,
};
use octez::r#async::rollup::{OctezRollupConfigBuilder, RollupDataDir};
use octez::unused_port;
use serde_json::Value;
use std::fs;
use tempfile::{NamedTempFile, TempDir};
use tezos_crypto_rs::hash::SmartRollupHash;
use tokio::time::{sleep, timeout};

const ACTIVATOR_PK: &str = "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2";
const CONTRACT_INIT_BALANCE: f64 = 1.0;
pub const JSTZ_ROLLUP_OPERATOR_PK: &str =
    "edpktoeTraS2WW9iaceu8hvHJ1sUJFSKavtzrMcp4jwMoJWWBts8AK";
pub const JSTZ_ROLLUP_OPERATOR_ALIAS: &str = "rollup_operator";

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[tokio::test(flavor = "multi_thread")]
async fn jstzd_test() {
    let octez_node_rpc_endpoint = Endpoint::localhost(unused_port());
    let rollup_rpc_endpoint = Endpoint::try_from(
        Uri::from_str(&format!("http://127.0.0.1:{}", unused_port())).unwrap(),
    )
    .unwrap();
    let jstz_node_rpc_endpoint = Endpoint::localhost(unused_port());
    let jstzd_port = unused_port();
    let (mut jstzd, config) = create_jstzd_server(
        &octez_node_rpc_endpoint,
        &rollup_rpc_endpoint,
        &jstz_node_rpc_endpoint,
        jstzd_port,
        #[cfg(feature = "oracle")]
        None,
    )
    .await;

    jstzd.run(false).await.unwrap();
    ensure_jstzd_components_are_up(&jstzd, &octez_node_rpc_endpoint, jstzd_port).await;

    match config
        .octez_rollup_config()
        .kernel_debug_file
        .as_ref()
        .unwrap()
        .as_ref()
    {
        FileWrapper::TempFile(v) => ensure_rollup_is_logging_to(v).await,
        _ => panic!("kernel_debug_file should be a temp file"),
    }

    let octez_client = OctezClient::new(config.octez_client_config().clone());
    check_bootstrap_contracts(&octez_client).await;

    fetch_config_test(config, jstzd_port).await;

    tokio::spawn(async move {
        sleep(Duration::from_secs(10)).await;
        reqwest::Client::new()
            .put(format!("http://localhost:{jstzd_port}/shutdown"))
            .send()
            .await
            .unwrap();
    });

    timeout(Duration::from_secs(30), jstzd.wait())
        .await
        .expect("should not wait too long for the server to be taken down");

    ensure_jstzd_components_are_down(&jstzd, &octez_node_rpc_endpoint, jstzd_port).await;

    // calling `run` after calling `stop` should fail because all states should have been cleared
    assert_eq!(
        jstzd.run(false).await.unwrap_err().to_string(),
        "cannot run jstzd server without jstzd config"
    );
}

async fn create_jstzd_server(
    octez_node_rpc_endpoint: &Endpoint,
    rollup_rpc_endpoint: &Endpoint,
    jstz_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
    #[cfg(feature = "oracle")] oracle_key_pair: Option<KeyPair>,
) -> (JstzdServer, JstzdConfig) {
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

    let protocol_params = ProtocolParameterBuilder::new()
        // this is the activator account
        // fund the account so that it can be used by the baker
        // give it at least 12000 (6000 for bootstrap + 6000 for baking) tez
        // so that it does not run out of fund
        .set_bootstrap_accounts([
            // activator is given at least 12000 (6000 for bootstrap + 6000 for baking) tez for baking
            BootstrapAccount::new(ACTIVATOR_PK, 15_000_000_000).unwrap(),
            BootstrapAccount::new(JSTZ_ROLLUP_OPERATOR_PK, 60_000_000_000).unwrap(),
        ])
        .set_bootstrap_smart_rollups([BootstrapSmartRollup::new(
            JSTZ_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Wasm,
            fs::read_to_string(rollup_kernel_installer.as_path())
                .unwrap()
                .as_str(),
            Value::from_str(fs::read_to_string(rollup_parameters_ty).unwrap().as_str())
                .expect("failed to stringify JSON"),
        )
        .unwrap()])
        .set_bootstrap_contracts(read_bootstrap_contracts().await)
        .build()
        .unwrap();
    let octez_client_config =
        OctezClientConfigBuilder::new(octez_node_config.rpc_endpoint.clone())
            .build()
            .unwrap();
    let baker_config = OctezBakerConfigBuilder::new()
        .set_binary_path(BakerBinaryPath::Env(protocol_params.protocol().clone()))
        .set_octez_client_base_dir(
            PathBuf::from(octez_client_config.base_dir())
                .to_str()
                .unwrap(),
        )
        .set_octez_node_endpoint(&octez_node_config.rpc_endpoint)
        .build()
        .expect("Failed to build baker config");
    let kernel_debug_file = FileWrapper::default();
    let kernel_debug_file_path = kernel_debug_file.path();
    let preimages_dir = TempDir::new().unwrap();
    let preimages_dir_path = preimages_dir.path().to_path_buf();
    let rollup_config = OctezRollupConfigBuilder::new(
        octez_node_rpc_endpoint.clone(),
        octez_client_config.base_dir().into(),
        SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        JSTZ_ROLLUP_OPERATOR_ALIAS.to_string(),
        rollup_kernel_installer,
        None,
    )
    .set_data_dir(RollupDataDir::TempWithPreImages {
        preimages_dir: rollup_preimages_dir,
    })
    .set_rpc_endpoint(rollup_rpc_endpoint)
    .set_kernel_debug_file(kernel_debug_file)
    .build()
    .expect("failed to build rollup config");
    let jstz_node_config = JstzNodeConfig::new(
        jstz_node_rpc_endpoint,
        &rollup_config.rpc_endpoint,
        &preimages_dir_path,
        &kernel_debug_file_path,
        KeyPair(
            PublicKey::from_base58(
                "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
            )
            .unwrap(),
        ),
        jstz_node::RunMode::Default,
        false,
    );
    let config = JstzdConfig::new(
        octez_node_config,
        baker_config,
        octez_client_config.clone(),
        rollup_config.clone(),
        #[cfg(feature = "oracle")]
        OracleNodeConfig {
            key_pair: oracle_key_pair,
            log_path: kernel_debug_file_path.clone(),
            jstz_node_endpoint: jstz_node_rpc_endpoint.to_owned(),
        },
        jstz_node_config,
        protocol_params,
    );
    (JstzdServer::new(config.clone(), jstzd_port), config)
}

async fn ensure_jstzd_components_are_up(
    jstzd: &JstzdServer,
    octez_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
) {
    let jstzd_health_check_endpoint = format!("http://localhost:{jstzd_port}/health");
    let octez_node_health_check_endpoint =
        format!("{octez_node_rpc_endpoint}/health/ready");

    let jstzd_running = retry(30, 1000, || async {
        let res = reqwest::get(&jstzd_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(jstzd_running);

    // check if individual components are up / jstzd health check indeed covers all components
    assert!(reqwest::get(&octez_node_health_check_endpoint)
        .await
        .is_ok());
    assert!(jstzd.baker_healthy().await);
    let rollup_running =
        retry(10, 1000, || async { Ok(jstzd.rollup_healthy().await) }).await;
    assert!(rollup_running);
    assert!(jstzd.jstz_node_healthy().await);
    assert!(jstzd.health_check().await);
}

async fn ensure_jstzd_components_are_down(
    jstzd: &JstzdServer,
    octez_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
) {
    let jstzd_health_check_endpoint = format!("http://localhost:{jstzd_port}/health");
    let octez_node_health_check_endpoint =
        format!("{octez_node_rpc_endpoint}/health/ready");

    let jstzd_stopped = retry(30, 1000, || async {
        let res = reqwest::get(&jstzd_health_check_endpoint).await;
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Ok(false)
    })
    .await;
    assert!(jstzd_stopped);

    // check if individual components are terminated
    // and jstzd indeed tears down all components before it shuts down
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
    assert!(!jstzd.baker_healthy().await);
    assert!(!jstzd.rollup_healthy().await);
    assert!(!jstzd.jstz_node_healthy().await);
    assert!(!jstzd.health_check().await);
}

async fn fetch_config_test(jstzd_config: JstzdConfig, jstzd_port: u16) {
    let mut full_config = serde_json::json!({});
    for (key, expected_json) in [
        (
            "octez_node",
            serde_json::to_value(jstzd_config.octez_node_config()).unwrap(),
        ),
        (
            "octez_client",
            serde_json::to_value(jstzd_config.octez_client_config()).unwrap(),
        ),
        (
            "octez_baker",
            serde_json::to_value(jstzd_config.baker_config()).unwrap(),
        ),
        (
            "octez_rollup",
            serde_json::to_value(jstzd_config.octez_rollup_config()).unwrap(),
        ),
        (
            "jstz_node",
            serde_json::to_value(jstzd_config.jstz_node_config()).unwrap(),
        ),
    ] {
        let res = reqwest::get(&format!("http://localhost:{jstzd_port}/config/{key}"))
            .await
            .unwrap();
        assert_eq!(
            expected_json,
            serde_json::from_str::<serde_json::Value>(&res.text().await.unwrap())
                .unwrap(),
            "config mismatch at /config/{key}"
        );
        full_config
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), expected_json);
    }

    // invalid config type
    assert_eq!(
        reqwest::get(&format!("http://localhost:{jstzd_port}/config/foobar"))
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );

    // all configs
    let res = reqwest::get(&format!("http://localhost:{jstzd_port}/config/"))
        .await
        .unwrap();
    assert_eq!(
        full_config,
        serde_json::from_str::<serde_json::Value>(&res.text().await.unwrap()).unwrap(),
        "config mismatch at /config/",
    );
}

fn jstz_rollup_files() -> (PathBuf, PathBuf, PathBuf) {
    (
        kernel_installer_path(),
        preimages_path(),
        parameters_ty_path(),
    )
}

async fn read_bootstrap_contracts() -> Vec<BootstrapContract> {
    let mut contracts = vec![];
    for (contract_name, hash) in BOOTSTRAP_CONTRACT_NAMES {
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
    for (contract_name, hash) in BOOTSTRAP_CONTRACT_NAMES {
        assert_eq!(
            octez_client
                .get_balance(hash)
                .await
                .unwrap_or_else(|_| panic!(
                    "should be able to find contract '{contract_name}' at '{hash}'"
                )),
            CONTRACT_INIT_BALANCE,
            "balance mismatch for contract '{contract_name}'"
        );
    }
}

async fn ensure_rollup_is_logging_to(kernel_debug_file: &NamedTempFile) {
    let mut file = kernel_debug_file.reopen().unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert!(contents.contains("Internal message: start of level"));
    assert!(contents.contains("Internal message: end of level"));
}

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[cfg(feature = "oracle")]
#[tokio::test(flavor = "multi_thread")]
async fn jstzd_with_oracle_key_pair_test() {
    let octez_node_rpc_endpoint = Endpoint::localhost(unused_port());
    let rollup_rpc_endpoint = Endpoint::try_from(
        Uri::from_str(&format!("http://127.0.0.1:{}", unused_port())).unwrap(),
    )
    .unwrap();
    let jstz_node_rpc_endpoint = Endpoint::localhost(unused_port());
    let jstzd_port = unused_port();

    let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
    let (oracle_pk, oracle_sk) = keypair_from_mnemonic(mnemonic, "").unwrap();

    let (mut jstzd, config) = create_jstzd_server(
        &octez_node_rpc_endpoint,
        &rollup_rpc_endpoint,
        &jstz_node_rpc_endpoint,
        jstzd_port,
        Some(KeyPair(oracle_pk, oracle_sk)),
    )
    .await;

    jstzd.run(false).await.unwrap();
    ensure_jstzd_components_are_up(&jstzd, &octez_node_rpc_endpoint, jstzd_port).await;

    let KeyPair(cfg_pk, cfg_sk) = config
        .oracle_node_config()
        .key_pair
        .as_ref()
        .expect("oracle key pair missing");
    assert_eq!(
        cfg_pk.to_string(),
        "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi"
    );
    assert_eq!(
        cfg_sk.to_string(),
        "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM"
    );

    // graceful shutdown
    tokio::spawn(async move {
        sleep(Duration::from_secs(3)).await;
        reqwest::Client::new()
            .put(format!("http://localhost:{}/shutdown", jstzd_port))
            .send()
            .await
            .unwrap();
    });

    timeout(Duration::from_secs(30), jstzd.wait())
        .await
        .expect("shutdown timed out");
    ensure_jstzd_components_are_down(&jstzd, &octez_node_rpc_endpoint, jstzd_port).await;
}
