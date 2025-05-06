use jstz_crypto::{
    hash::Blake2b,
    smart_function_hash::{Kt1Hash, SmartFunctionHash},
};
use jstz_proto::receipt::{DeployFunctionReceipt, Receipt};
use std::{fs::File, io::Write};
use tempfile::{NamedTempFile, TempDir};
use tezos_crypto_rs::hash::ContractKt1Hash;
use utils::jstz_cmd;

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
fn deploy() {
    let mut server = mockito::Server::new();
    server
        .mock(
            "GET",
            "/accounts/tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6/nonce",
        )
        .with_body("0")
        .create();
    server.mock("POST", "/operations").with_status(200).create();
    let receipt = Receipt::new(
        Blake2b::default(),
        Ok(jstz_proto::receipt::ReceiptContent::DeployFunction(
            DeployFunctionReceipt {
                address: SmartFunctionHash(Kt1Hash(
                    ContractKt1Hash::from_base58_check(
                        "KT19GXucGUitURBXXeEMMfqqhSQ5byt4P1zX",
                    )
                    .unwrap(),
                )),
            },
        )),
    );
    server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/operations/\w+/receipt$".to_string()),
        )
        .with_body(serde_json::to_string(&receipt).unwrap())
        .create();

    let success_msg = "Smart function deployed by test_user at address: KT1";
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(file, &config(&server.url()))
        .expect("should write config file");
    let mut source_file = NamedTempFile::new().unwrap();
    source_file
        .write_all("export default (() => new Response('hello world'))".as_bytes())
        .unwrap();

    let mut process = jstz_cmd(
        [
            "deploy",
            source_file.path().to_str().unwrap(),
            "--name",
            "dummy",
        ],
        Some(tmp_dir),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains(success_msg));

    let mut process = jstz_cmd(
        [
            "deploy",
            source_file.path().to_str().unwrap(),
            "--name",
            "dummy",
        ],
        Some(process.tmp),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains(
        "The name 'dummy' is already used by another smart function or a user account."
    ));

    let mut process = jstz_cmd(
        [
            "deploy",
            source_file.path().to_str().unwrap(),
            "--name",
            "dummy",
            "--force",
        ],
        Some(process.tmp),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains(success_msg));

    // a new name with force should work
    let mut process = jstz_cmd(
        [
            "deploy",
            source_file.path().to_str().unwrap(),
            "--name",
            "dummy-new",
            "--force",
        ],
        Some(process.tmp),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains(success_msg));

    // force without a name should work
    let mut process = jstz_cmd(
        ["deploy", source_file.path().to_str().unwrap(), "--force"],
        Some(process.tmp),
    );
    let output = process.exp_eof().unwrap();
    assert!(output.contains(success_msg));
}
