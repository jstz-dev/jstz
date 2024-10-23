use std::path::Path;

use jstzd::{
    protocol::Protocol,
    task::{
        octez_baker::{BakerBinaryPath, OctezBaker, OctezBakerConfigBuilder},
        octez_client::{OctezClient, OctezClientBuilder},
        octez_node, Task,
    },
};
use octez::{OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder};
use regex::Regex;
use tempfile::TempDir;
mod utils;
use utils::{get_request, retry};

#[tokio::test(flavor = "multi_thread")]
async fn test_baker() {
    // 1. start octez node
    let (mut octez_node, _temp_data_dir) = spawn_octez_node().await;
    // 2. setup octez client
    let temp_dir = TempDir::new().unwrap();
    let base_dir: std::path::PathBuf = temp_dir.path().to_path_buf();
    let node_endpoint = octez_node.rpc_endpoint().clone();
    let octez_client = OctezClientBuilder::new()
        .set_endpoint(node_endpoint.clone())
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    // 3. activate alpha protocol, the block level is 1 after activation
    activate_alpha(&octez_client).await;
    import_bootstrap_keys(&octez_client).await;
    // 4. start baker
    let baker_config = OctezBakerConfigBuilder::new()
        .set_binary_path(BakerBinaryPath::BuiltIn(Protocol::Alpha))
        .with_node_and_client(&octez_node, &octez_client)
        .build()
        .expect("Failed to build baker config");
    // check if the block is baked
    let mut baker_node = OctezBaker::spawn(baker_config).await.expect("SHOULD RUN");
    assert!(baker_node.health_check().await.unwrap());
    let block_baked = retry(10, 1000, || async {
        let level = get_block_level(&node_endpoint.to_string()).await;
        Ok(level > 1)
    })
    .await;
    assert!(block_baked);
    // 5. kill the baker node
    let _ = baker_node.kill().await;
    assert!(!baker_node.health_check().await.unwrap());
    // check if the block level stops increasing after killing
    let last_level = get_block_level(&node_endpoint.to_string()).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let current_level = get_block_level(&node_endpoint.to_string()).await;
    assert_eq!(last_level, current_level);
    let _ = octez_node.kill().await;
}

async fn spawn_octez_node() -> (octez_node::OctezNode, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path();
    let mut config_builder = OctezNodeConfigBuilder::new();
    let run_options = OctezNodeRunOptionsBuilder::new()
        .set_synchronisation_threshold(0)
        .build();
    config_builder
        .set_binary_path("octez-node")
        .set_network("sandbox")
        .set_data_dir(data_dir.to_str().unwrap())
        .set_run_options(&run_options);
    let octez_node = octez_node::OctezNode::spawn(config_builder.build().unwrap())
        .await
        .unwrap();
    let node_ready = retry(10, 1000, || async { octez_node.health_check().await }).await;
    assert!(node_ready);
    (octez_node, temp_dir)
}

async fn import_bootstrap_keys(octez_client: &OctezClient) {
    for (idx, key) in [
        "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
        "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
        "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
    ]
    .iter()
    .enumerate()
    {
        let alias = format!("bootstrap{}", idx + 1);
        octez_client
            .import_secret_key(&alias, key)
            .await
            .expect("Failed to generate bootstrap key");
    }
}

async fn activate_alpha(octez_client: &OctezClient) {
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
        .activate_protocol(
            "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            "0",
            &activator,
            &params_file,
        )
        .await;
    assert!(protocol_activated.is_ok());
}

async fn get_block_level(rpc_endpoint: &str) -> i32 {
    let blocks_head_endpoint =
        format!("{}/chains/main/blocks/head", rpc_endpoint.to_owned());
    let response = get_request(&blocks_head_endpoint).await;
    extract_level(&response)
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
