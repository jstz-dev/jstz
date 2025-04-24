use std::fs::File;

use tempfile::TempDir;
use utils::jstz;

#[path = "./utils.rs"]
mod utils;

fn config(endpoint: &str) -> serde_json::Value {
    serde_json::json!({
        "current_alias": "test_user",
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
fn run_with_invalid_scheme() {
    let mut server = mockito::Server::new();
    server
        .mock(
            "GET",
            "/accounts/tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6/nonce",
        )
        .with_body("0")
        .create();

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(file, &config(&server.url()))
        .expect("should write config file");

    let mut process = jstz(
        "run tezos://tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6 -n test",
        Some(tmp_dir),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains("URL scheme must be 'jstz'"));
}
