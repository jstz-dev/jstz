use std::{fs::File, str::FromStr};

use octez::r#async::{
    client::{OctezClient, OctezClientConfigBuilder},
    endpoint::Endpoint,
    protocol::Protocol,
};
use tempfile::TempDir;
use utils::jstz;

#[path = "./utils.rs"]
mod utils;

fn config(endpoint: &str, octez_client_dir: &str) -> serde_json::Value {
    serde_json::json!({
        "current_alias": "test_user",
        "octez_client_dir": octez_client_dir,
        "accounts": {
            "test_user": {
                "User": {
                    "address": "tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6",
                    "secret_key": "edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf",
                    "public_key": "edpktpcAZ3d8Yy1EZUF1yX4xFgLq5sJ7cL9aVhp7aV12y89RXThE3N"
                }
            },
        },
        "default_network": "test",
        "networks": {"test": {"octez_node_rpc_endpoint": endpoint, "jstz_node_endpoint": endpoint}},
    })
}

#[test]
fn withdraw() {
    let mut server = mockito::Server::new();
    server
        .mock(
            "GET",
            "/accounts/tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6/nonce",
        )
        .with_body("0")
        .create();
    // to update the operation hash, run the test with JSTZ_LOG=debug and read the new hash
    server
        .mock("GET", "/operations/33f21024bf17363666aeea641113af804912b8f1f76a18bbd2414de9b943cfdf/receipt")
        .with_status(200)
        .with_header("content-type", "application/json")
        // hash in response body does not matter
        .with_body(
            r#"{
            "hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "result": {
                "_type": "Success",
                "inner": { "_type": "RunFunction", "statusCode": 200, "headers": {}}
            }
        }"#,
        )
        .create();
    server.mock("POST", "/operations").with_status(200).create();

    // Mock endpoint for octez client to read basic info
    let protocol = Protocol::Alpha.hash();
    server
        .mock("GET", "/chains/main/blocks/head/protocols")
        .with_body(
            serde_json::json!({"protocol": protocol, "next_protocol": protocol})
                .to_string(),
        )
        .create();

    // Mock octez client base dir with the dummy L1 account
    let octez_client_dir = TempDir::new().unwrap();
    let client = OctezClient::new(
        OctezClientConfigBuilder::new(Endpoint::from_str(&server.url()).unwrap())
            .set_base_dir(octez_client_dir.path().to_path_buf())
            .build()
            .unwrap(),
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(client.import_secret_key(
            "dummy",
            "unencrypted:edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        ))
        .unwrap();

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(
        file,
        &config(&server.url(), octez_client_dir.path().to_str().unwrap()),
    )
    .expect("should write config file");

    let mut process = jstz(
        "bridge withdraw --to dummy --amount 10 -n test",
        Some(tmp_dir),
    );
    let output = process.exp_eof().unwrap();
    // withdraw does not print any confirmation
    assert_eq!(output, "");
}
