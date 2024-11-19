mod utils;
use std::path::PathBuf;

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
    let run_options = OctezNodeRunOptionsBuilder::new()
        .set_synchronisation_threshold(0)
        .set_network("sandbox")
        .build();
    let octez_node_config = OctezNodeConfigBuilder::new()
        .set_network("sandbox")
        .set_rpc_endpoint(&rpc_endpoint)
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

    let config = jstzd::task::jstzd::JstzdConfig::new(
        octez_node_config,
        baker_config,
        octez_client_config.clone(),
        protocol_params,
    );
    let jstzd_port = unused_port();
    let mut jstzd = jstzd::task::jstzd::JstzdServer::new(config, jstzd_port);
    jstzd.run().await.unwrap();

    let jstz_health_check_endpoint = format!("http://localhost:{}/health", jstzd_port);
    let octez_node_health_check_endpoint = format!("{}/health/ready", rpc_endpoint);
    let jstzd_running = retry(30, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(jstzd_running);

    let node_running = retry(30, 1000, || async {
        let res = reqwest::get(&octez_node_health_check_endpoint).await;
        Ok(res.is_ok())
    })
    .await;
    assert!(node_running);

    assert!(jstzd.baker_healthy().await);
    assert!(jstzd.health_check().await);

    let octez_client = OctezClient::new(octez_client_config);
    check_bootstrap_contracts(&octez_client).await;

    reqwest::Client::new()
        .put(&format!("http://localhost:{}/shutdown", jstzd_port))
        .send()
        .await
        .unwrap();

    let jstzd_stopped = retry(30, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Ok(false)
    })
    .await;
    assert!(jstzd_stopped);

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

    // stop should be idempotent and thus should be okay after jstzd is already stopped
    jstzd.stop().await.unwrap();
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
