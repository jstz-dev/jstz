use jstzd::task::{endpoint::Endpoint, octez_client::OctezClientBuilder};
use serde_json::Value;
use std::{
    fs::{read_to_string, remove_file},
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};

fn read_file(path: &Path) -> Value {
    serde_json::from_str(&read_to_string(path).expect("Unable to read file"))
        .expect("Unable to parse JSON")
}

fn first_item(json: Value) -> Value {
    json.as_array().unwrap()[0].clone()
}

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
    assert!(res.is_err_and(|e| e.to_string().contains("failed to generate key")));
}
