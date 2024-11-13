#![allow(dead_code)]
use jstzd::task::{octez_baker, octez_node::OctezNode, octez_rollup, utils::retry, Task};
use octez::r#async::{
    baker::{BakerBinaryPath, OctezBakerConfigBuilder},
    client::{OctezClient, OctezClientConfigBuilder},
    endpoint::Endpoint,
    node_config::{OctezNodeConfigBuilder, OctezNodeRunOptionsBuilder},
    protocol::Protocol,
    rollup::{OctezRollupConfigBuilder, RollupDataDir},
};
use regex::Regex;
use std::path::{Path, PathBuf};
use tezos_crypto_rs::hash::{BlockHash, OperationHash, SmartRollupHash};
use tokio::io::AsyncReadExt;

pub const ACTIVATOR_SECRET_KEY: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";
pub const ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";

pub async fn setup() -> (OctezNode, OctezClient, octez_baker::OctezBaker) {
    let octez_node = spawn_octez_node().await;
    let octez_client = create_client(octez_node.rpc_endpoint());

    import_bootstrap_keys(&octez_client).await;
    import_activator(&octez_client).await;
    activate_alpha(&octez_client, None).await;

    let baker = spawn_baker(&octez_node, &octez_client).await;
    (octez_node, octez_client, baker)
}

pub async fn spawn_rollup(
    octez_node: &OctezNode,
    octez_client: &OctezClient,
) -> octez_rollup::OctezRollup {
    let kernel_installer = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join("tests/toy_rollup/kernel_installer");
    let preimages_dir =
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/toy_rollup/preimages");
    let config = OctezRollupConfigBuilder::new(
        octez_node.rpc_endpoint().clone(),
        octez_client.base_dir().into(),
        SmartRollupHash::from_base58_check(ROLLUP_ADDRESS).unwrap(),
        "bootstrap1".to_string(),
        kernel_installer,
    )
    .set_data_dir(RollupDataDir::TempWithPreImages { preimages_dir })
    .build()
    .unwrap();
    let rollup = octez_rollup::OctezRollup::spawn(config)
        .await
        .expect("Failed to spawn rollup");
    let rollup_ready = retry(10, 1000, || async { rollup.health_check().await }).await;
    assert!(rollup_ready);
    rollup
}

pub async fn spawn_baker(
    octez_node: &OctezNode,
    octez_client: &OctezClient,
) -> octez_baker::OctezBaker {
    let baker_config = OctezBakerConfigBuilder::new()
        .set_binary_path(BakerBinaryPath::Env(Protocol::Alpha))
        .set_octez_client_base_dir(
            PathBuf::from(octez_client.base_dir()).to_str().unwrap(),
        )
        .set_octez_node_endpoint(octez_node.rpc_endpoint())
        .build()
        .expect("Failed to build baker config");
    // check if the block is baked
    let baker_node = octez_baker::OctezBaker::spawn(baker_config)
        .await
        .expect("SHOULD RUN");
    assert!(baker_node.health_check().await.unwrap());
    let node_endpoint = octez_node.rpc_endpoint();
    let block_baked = retry(10, 1000, || async {
        let level = get_block_level(&node_endpoint.to_string()).await;
        Ok(level > 1)
    })
    .await;
    assert!(block_baked);
    baker_node
}

pub async fn spawn_octez_node() -> OctezNode {
    let mut config_builder = OctezNodeConfigBuilder::new();
    let mut run_option_builder = OctezNodeRunOptionsBuilder::new();
    config_builder
        .set_binary_path("octez-node")
        .set_network("sandbox")
        .set_run_options(&run_option_builder.set_synchronisation_threshold(0).build());
    let octez_node = OctezNode::spawn(config_builder.build().unwrap())
        .await
        .unwrap();
    let node_ready = retry(10, 1000, || async { octez_node.health_check().await }).await;
    assert!(node_ready);
    octez_node
}

pub fn create_client(node_endpoint: &Endpoint) -> OctezClient {
    let config = OctezClientConfigBuilder::new(node_endpoint.clone())
        .build()
        .unwrap();
    OctezClient::new(config)
}

pub async fn get_block_level(rpc_endpoint: &str) -> i32 {
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

pub async fn get_head_block_hash(rpc_endpoint: &str) -> BlockHash {
    let blocks_head_endpoint =
        format!("{}/chains/main/blocks/head", rpc_endpoint.to_owned());
    let response = get_request(&blocks_head_endpoint).await;
    BlockHash::from_base58_check(
        serde_json::from_str::<serde_json::Value>(&response)
            .unwrap()
            .as_object()
            .unwrap()
            .get("hash")
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

pub async fn import_bootstrap_keys(octez_client: &OctezClient) {
    for (idx, key) in [
        "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
        "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
        "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
        "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
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

pub async fn import_activator(octez_client: &OctezClient) {
    let activator = "activator".to_string();
    octez_client
        .import_secret_key(&activator, ACTIVATOR_SECRET_KEY)
        .await
        .expect("Failed to generate activator key");
}

pub async fn activate_alpha(octez_client: &OctezClient, params: Option<PathBuf>) {
    let params_file = params.unwrap_or(
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/sandbox-params.json"),
    );
    let protocol_activated = octez_client
        .activate_protocol(
            "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            "0",
            "activator",
            &params_file,
        )
        .await;
    assert!(protocol_activated.is_ok());
}

pub async fn get_request(endpoint: &str) -> String {
    reqwest::get(endpoint).await.unwrap().text().await.unwrap()
}

pub async fn get_operation_kind(
    rpc_endpoint: &str,
    block_hash: &BlockHash,
    operation_hash: &OperationHash,
) -> Option<String> {
    let op_str = operation_hash.to_string();
    let blocks_head_endpoint = format!(
        "{}/chains/main/blocks/{}/operations",
        rpc_endpoint.to_owned(),
        block_hash
    );
    let response: serde_json::Value =
        serde_json::from_str(&get_request(&blocks_head_endpoint).await).unwrap();
    for group in response.as_array()? {
        for op in group.as_array()? {
            let obj = op.as_object()?;
            if obj.get("hash")? == &op_str {
                return Some(
                    obj.get("contents")?
                        .as_array()?
                        .first()?
                        .get("kind")?
                        .as_str()?
                        .to_owned(),
                );
            }
        }
    }
    None
}

pub async fn read_json_file(path: PathBuf) -> serde_json::Value {
    let mut current_string = String::new();
    tokio::fs::File::open(&path)
        .await
        .map_err(|e| anyhow::anyhow!("error reading file '{}': {:?}", path.display(), e))
        .unwrap()
        .read_to_string(&mut current_string)
        .await
        .unwrap();
    serde_json::from_str(&current_string).unwrap()
}
