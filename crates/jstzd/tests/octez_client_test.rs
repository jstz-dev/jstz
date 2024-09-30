use jstzd::task::{endpoint::Endpoint, octez_client::OctezClientBuilder};
use serde_json::Value;
use std::fs::{read_to_string, remove_file};
use tempfile::{NamedTempFile, TempDir};

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
