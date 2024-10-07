use jstzd::task::{
    endpoint::Endpoint,
    octez_client::OctezClientBuilder,
    octez_node::{self, DEFAULT_RPC_ENDPOINT},
    Task,
};
use serde_json::Value;
use std::{
    fs::{read_to_string, remove_file},
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};
mod utils;
use serial_test::serial;
use utils::retry;

fn read_file(path: &Path) -> Value {
    serde_json::from_str(&read_to_string(path).expect("Unable to read file"))
        .expect("Unable to parse JSON")
}

fn first_item(json: Value) -> Value {
    json.as_array().unwrap()[0].clone()
}

const SECRET_KEY: &str =
    "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";

#[tokio::test]
async fn config_init() {
    let temp_dir = TempDir::new().unwrap();
    let expected_base_dir = temp_dir.path().to_path_buf();
    let expected_endpoint: Endpoint = Endpoint::localhost(3000);
    let config_file = NamedTempFile::new().unwrap();
    let _ = remove_file(config_file.path());
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(expected_base_dir.clone())
        .set_endpoint(expected_endpoint.clone())
        .build()
        .unwrap();
    let res = octez_client.config_init(config_file.path()).await;
    assert!(res.is_ok());
    let actual: Value =
        serde_json::from_str(&read_to_string(config_file).expect("Unable to read file"))
            .expect("Unable to parse JSON");
    assert_eq!(
        actual["base_dir"],
        expected_base_dir.to_str().unwrap().to_owned()
    );
    assert_eq!(actual["endpoint"], expected_endpoint.to_string());
}

#[tokio::test]
async fn generates_keys() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    let alias = "test_alias".to_string();
    let res = octez_client.gen_keys(&alias, None).await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert_eq!(secret_keys["name"], alias);
}

#[tokio::test]
async fn generates_keys_with_custom_signature() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    let alias = "test_alias".to_string();
    let res = octez_client
        .gen_keys(&alias, Some(jstzd::task::octez_client::Signature::BLS))
        .await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert!(pub_keys["value"]
        .as_str()
        .unwrap()
        .starts_with("unencrypted:BL"));
    assert_eq!(secret_keys["name"], alias);
    assert!(secret_keys["value"]
        .as_str()
        .unwrap()
        .starts_with("unencrypted:BL"));
}

#[tokio::test]
async fn generates_keys_throws() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    let alias = "test_alias".to_string();
    let _ = octez_client.gen_keys(&alias, None).await;
    let res = octez_client.gen_keys(&alias, None).await;
    assert!(res.is_err_and(|e| { e.to_string().contains("\"gen\" \"keys\"") }));
}

#[tokio::test]
async fn imports_secret_key() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    let alias = "test_alias".to_string();
    let res = octez_client.import_secret_key(&alias, SECRET_KEY).await;
    assert!(res.is_ok());
    let hashes = first_item(read_file(&base_dir.join("public_key_hashs")));
    let pub_keys = first_item(read_file(&base_dir.join("public_keys")));
    let secret_keys = first_item(read_file(&base_dir.join("secret_keys")));
    assert_eq!(hashes["name"], alias);
    assert_eq!(pub_keys["name"], alias);
    assert_eq!(secret_keys["name"], alias);
}

#[tokio::test]
async fn imports_secret_key_throws() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    let alias = "test_alias".to_string();
    let _ = octez_client.import_secret_key(&alias, SECRET_KEY).await;
    let res = octez_client.import_secret_key(&alias, SECRET_KEY).await;
    assert!(
        res.is_err_and(|e| { e.to_string().contains("\"import\" \"secret\" \"key\"") })
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn activate_protocol() {
    // 1. start octez node
    let (mut octez_node, _temp_data_dir) = spawn_octez_node().await;
    // 2. setup octez client
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    let octez_client = OctezClientBuilder::new()
        .set_base_dir(base_dir.clone())
        .build()
        .unwrap();
    // 3. import activator key
    let activator = "activator".to_string();
    octez_client
        .import_secret_key(&activator, SECRET_KEY)
        .await
        .expect("Failed to generate activator key");
    let params_file =
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/sandbox-params.json");
    let blocks_head_endpoint =
        format!("http://{}/chains/main/blocks/head", DEFAULT_RPC_ENDPOINT);
    let response = get_response_text(&blocks_head_endpoint).await;
    assert!(response.contains(
        "\"protocol\":\"PrihK96nBAFSxVL1GLJTVhu9YnzkMFiBeuJRPA8NwuZVZCE1L6i\""
    ));
    assert!(response.contains("\"level\":0"));
    // 4. activate the alpha protocol
    let protocol_activated = octez_client
        .activate_protocol(
            "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            "0",
            &activator,
            &params_file,
        )
        .await;
    assert!(protocol_activated.is_ok());
    // 5. check if the protocol is activated and the block is baked.
    let response = get_response_text(&blocks_head_endpoint).await;
    assert!(response.contains(
        "\"protocol\":\"ProtoGenesisGenesisGenesisGenesisGenesisGenesk612im\""
    ));
    assert!(response.contains("\"level\":1"));
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
    let node_ready = retry(15, 1000, || async { octez_node.health_check().await }).await;
    assert!(node_ready);
    (octez_node, temp_dir)
}

async fn get_response_text(endpoint: &str) -> String {
    reqwest::get(endpoint)
        .await
        .expect("Failed to get block head")
        .text()
        .await
        .expect("Failed to get response text")
}
