mod utils;
use std::path::PathBuf;

use jstzd::task::jstzd::{JstzdConfig, JstzdServer};
use jstzd::task::utils::retry;
use jstzd::{EXCHANGER_ADDRESS, JSTZ_NATIVE_BRIDGE_ADDRESS};
use octez::r#async::baker::{BakerBinaryPath, OctezBakerConfigBuilder};
use octez::r#async::client::{OctezClient, OctezClientConfigBuilder};
use octez::r#async::endpoint::Endpoint;
use octez::r#async::node_config::{OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder};
use octez::r#async::protocol::{
    BootstrapAccount, BootstrapContract, ProtocolParameterBuilder,
};
use octez::unused_port;

const CONTRACT_INIT_BALANCE: f64 = 1.0;
const CONTRACT_NAMES: [(&str, &str); 2] = [
    ("exchanger", EXCHANGER_ADDRESS),
    ("jstz_native_bridge", JSTZ_NATIVE_BRIDGE_ADDRESS),
];

#[tokio::test(flavor = "multi_thread")]
async fn jstzd_test() {
    let rpc_endpoint = Endpoint::localhost(unused_port());
    let jstzd_port = unused_port();
    let (mut jstzd, config) = create_jstzd_server(&rpc_endpoint, jstzd_port).await;

    jstzd.run().await.unwrap();
    ensure_jstzd_components_are_up(&jstzd, &rpc_endpoint, jstzd_port).await;
    let octez_client = OctezClient::new(config.octez_client_config().clone());
    check_bootstrap_contracts(&octez_client).await;

    fetch_config_test(config, jstzd_port).await;

    reqwest::Client::new()
        .put(&format!("http://localhost:{}/shutdown", jstzd_port))
        .send()
        .await
        .unwrap();

    ensure_jstzd_components_are_down(&jstzd, &rpc_endpoint, jstzd_port).await;

    // calling `run` after calling `stop` should fail because all states should have been cleared
    assert_eq!(
        jstzd.run().await.unwrap_err().to_string(),
        "cannot run jstzd server without jstzd config"
    );
}

async fn create_jstzd_server(
    octez_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
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
    let protocol_params = ProtocolParameterBuilder::new()
        // this is the activator account
        // fund the account so that it can be used by the baker
        // give it at least 12000 (6000 for bootstrap + 6000 for baking) tez
        // so that it does not run out of fund
        .set_bootstrap_accounts([BootstrapAccount::new(
            "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
            15_000_000_000,
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

    let config = JstzdConfig::new(
        octez_node_config,
        baker_config,
        octez_client_config.clone(),
        protocol_params,
    );
    (JstzdServer::new(config.clone(), jstzd_port), config)
}

async fn ensure_jstzd_components_are_up(
    jstzd: &JstzdServer,
    octez_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
) {
    let jstz_health_check_endpoint = format!("http://localhost:{}/health", jstzd_port);
    let octez_node_health_check_endpoint =
        format!("{}/health/ready", octez_node_rpc_endpoint);

    let jstzd_running = retry(30, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(jstzd_running);

    // check if individual components are up / jstzd health check indeed covers all components
    assert!(reqwest::get(&octez_node_health_check_endpoint)
        .await
        .is_ok());

    assert!(jstzd.baker_healthy().await);
    assert!(jstzd.health_check().await);
}

async fn ensure_jstzd_components_are_down(
    jstzd: &JstzdServer,
    octez_node_rpc_endpoint: &Endpoint,
    jstzd_port: u16,
) {
    let jstz_health_check_endpoint = format!("http://localhost:{}/health", jstzd_port);
    let octez_node_health_check_endpoint =
        format!("{}/health/ready", octez_node_rpc_endpoint);

    let jstzd_stopped = retry(30, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
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
    assert!(!jstzd.health_check().await);
}

async fn fetch_config_test(jstzd_config: JstzdConfig, jstzd_port: u16) {
    let mut full_config = serde_json::json!({});
    for (key, expected_json) in [
        (
            "octez-node",
            serde_json::to_value(jstzd_config.octez_node_config()).unwrap(),
        ),
        (
            "octez-client",
            serde_json::to_value(jstzd_config.octez_client_config()).unwrap(),
        ),
        (
            "octez-baker",
            serde_json::to_value(jstzd_config.baker_config()).unwrap(),
        ),
    ] {
        let res =
            reqwest::get(&format!("http://localhost:{}/config/{}", jstzd_port, key))
                .await
                .unwrap();
        assert_eq!(
            expected_json,
            serde_json::from_str::<serde_json::Value>(&res.text().await.unwrap())
                .unwrap(),
            "config mismatch at /config/{}",
            key
        );
        full_config
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), expected_json);
    }

    // invalid config type
    assert_eq!(
        reqwest::get(&format!("http://localhost:{}/config/foobar", jstzd_port))
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );

    // all configs
    let res = reqwest::get(&format!("http://localhost:{}/config/", jstzd_port))
        .await
        .unwrap();
    assert_eq!(
        full_config,
        serde_json::from_str::<serde_json::Value>(&res.text().await.unwrap()).unwrap(),
        "config mismatch at /config/",
    );
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
