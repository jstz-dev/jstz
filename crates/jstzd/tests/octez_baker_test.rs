use std::{path::Path, str::FromStr};

use http::Uri;
use jstzd::task::{
    endpoint::Endpoint,
    octez_baker::{OctezBaker, OctezBakerConfigBuilder, Protocol},
    octez_client::{OctezClient, OctezClientBuilder},
    octez_node, Task,
};
use regex::Regex;
use tempfile::TempDir;
use tokio::time::sleep;
mod utils;
use utils::{get_request, retry};

#[tokio::test(flavor = "multi_thread")]
async fn baker() {
    // 1. start octez node
    let (mut octez_node, _temp_data_dir) = spawn_octez_node().await;
    // 2. setup octez client
    let temp_dir = TempDir::new().unwrap();
    let base_dir: std::path::PathBuf = temp_dir.path().to_path_buf();
    let rpc_endpoint = Uri::from_str(octez_node.rpc_endpoint()).unwrap();
    let rpc_endpoint: Endpoint = Endpoint::try_from(rpc_endpoint).unwrap();
    let octez_client = OctezClientBuilder::new()
        .set_endpoint(rpc_endpoint.clone())
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    // 3. activate alpha protocol, the block level is 1 after activation
    activate_protocol(
        &octez_client,
        "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
    )
    .await;
    import_bootstrap_keys(&octez_client).await;
    // 5. start baker
    let baker_config = OctezBakerConfigBuilder::new()
        .set_protocol(Protocol::Alpha)
        .with_node_and_client(&octez_node.config, &octez_client)
        .build()
        .expect("Failed to build baker config");
    let mut baker = OctezBaker::spawn(baker_config).await.expect("SHOULD RUN");
    sleep(tokio::time::Duration::from_secs(3)).await;
    let blocks_head_endpoint =
        format!("{}/chains/main/blocks/head", rpc_endpoint.to_string());
    let response = get_request(&blocks_head_endpoint).await;
    let level = extract_level(&response);
    assert!(level > 1);
    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}

async fn spawn_octez_node() -> (octez_node::OctezNode, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path();
    let mut config_builder = octez_node::OctezNodeConfigBuilder::new();
    config_builder
        .set_binary_path("octez-node")
        .set_network("sandbox")
        .set_data_dir(data_dir.to_str().unwrap())
        .set_run_options(&["--synchronisation-threshold", "0"]);
    let octez_node = octez_node::OctezNode::spawn(config_builder.build().unwrap())
        .await
        .unwrap();
    let node_ready = retry(10, 1000, || async { octez_node.health_check().await }).await;
    assert!(node_ready);
    (octez_node, temp_dir)
}

async fn import_bootstrap_keys(octez_client: &OctezClient) {
    for i in 1..5 {
        let alias = format!("bootstrap{}", i);
        let key = match i {
            1 => "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            2 => "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
            3 => "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
            4 => "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
            _ => panic!("Invalid key"),
        };
        octez_client
            .import_secret_key(&alias, key)
            .await
            .expect("Failed to generate bootstrap key");
    }
}

async fn activate_protocol(octez_client: &OctezClient, protocol: &str) {
    // 3. import activator key
    let activator = "activator".to_string();
    octez_client
        .import_secret_key(
            &activator,
            "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
        )
        .await
        .expect("Failed to generate activator key");
    // 4. activate the alpha protocol
    let params_file =
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/sandbox-params.json");
    let protocol_activated = octez_client
        .activate_protocol(protocol, "0", &activator, &params_file)
        .await;
    assert!(protocol_activated.is_ok());
}

fn extract_level(input: &str) -> i32 {
    // Create a regex to match "level": followed by a number
    let re = Regex::new(r#""level":\s*(\d+)"#).unwrap();
    // Extract the number as a string and parse it to i32
    re.captures(input)
        .unwrap()
        .get(1)
        .map(|level_match| level_match.as_str().parse::<i32>().unwrap())
        .unwrap()
}
