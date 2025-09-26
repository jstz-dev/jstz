use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use predicates::prelude::PredicateBooleanExt;
use std::{
    fs::{create_dir_all, File},
    io::Write,
    process::Command,
};
use tempfile::TempDir;
#[path = "./utils.rs"]
mod utils;

#[test]
fn list_networks() {
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    create_dir_all(path.parent().expect("should find parent dir"))
        .expect("should create dir");
    let mut file = File::create(&path).expect("should create file");
    file.write_all(
        serde_json::json!({
            "networks": {
                "foo": {
                    "octez_node_rpc_endpoint": "http://octez.foo.test",
                    "jstz_node_endpoint": "http://jstz.foo.test"
                },
                "very_long_name_very_long_name_very_long_name": {
                    "octez_node_rpc_endpoint": "http://octez.long.long.long.long.test",
                    "jstz_node_endpoint": "http://jstz.long.long.long.long.test"
                }
            }
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    file.flush().unwrap();

    let mut process = utils::jstz_cmd(["network", "list"], Some(tmp_dir));
    let output = process.exp_eof().unwrap().replace("\r\n", "\n");
    assert_eq!(
        output,
        r#"  +----------------------+---------------------------+---------------------------+
  | Name                 | Octez RPC endpoint        | Jstz node endpoint        |
  +======================+===========================+===========================+
  | foo                  | http://octez.foo.test     | http://jstz.foo.test      |
  +----------------------+---------------------------+---------------------------+
  | very_long_name_ve... | http://octez.long.long... | http://jstz.long.long.... |
  +----------------------+---------------------------+---------------------------+

"#
    );
}

#[test]
fn add_network() {
    let network_name = "a".repeat(25);
    let short_name = "aaaaaaaaaaaaaaaaa...";
    let tmp_dir = TempDir::new().unwrap();
    let home_path = tmp_dir.path().to_string_lossy().to_string();
    let path = tmp_dir.path().join(".config/jstz/config.json");
    create_dir_all(path.parent().expect("should find parent dir"))
        .expect("should create dir");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(file, &serde_json::json!({}))
        .expect("should write config file");

    // network does not exist yet
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(predicates::str::contains(&network_name).not())
        .success();

    // missing args
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "add", &network_name])
        .assert()
        .stderr(
            predicates::str::contains(
                "the following required arguments were not provided",
            )
            .and(predicates::str::contains(
                "--octez-node-rpc-endpoint <OCTEZ_NODE_RPC_ENDPOINT>",
            )),
        )
        .failure();

    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "add",
            &network_name,
            "--octez-node-rpc-endpoint",
            "http://octez.test",
            "--jstz-node-endpoint",
            "http://jstz.test",
        ])
        .assert()
        .stderr(predicates::str::contains(format!(
            "Added network '{short_name}'."
        )))
        .success();

    // network should be listed
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(short_name)
                .and(predicates::str::contains("http://octez.test")),
        )
        .success();

    let mut new_args = vec![
        "network",
        "add",
        &network_name,
        "--octez-node-rpc-endpoint",
        "http://new.octez.test",
        "--jstz-node-endpoint",
        "http://new.jstz.test",
    ];
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(&new_args)
        .assert()
        .stderr(predicates::str::contains(format!(
            "Network '{short_name}' already exists. Use `--force` to overwrite the network.",
        )))
        .failure();

    new_args.push("--force");
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(&new_args)
        .assert()
        .stderr(predicates::str::contains(format!(
            "Added network '{short_name}'."
        )))
        .success();

    // network should be listed with new endpoints
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(short_name)
                .and(predicates::str::contains("http://new.octez.test")),
        )
        .success();
}

#[test]
fn update_network() {
    let network_name = "foo";
    let tmp_dir = TempDir::new().unwrap();
    let home_path = tmp_dir.path().to_string_lossy().to_string();
    let path = tmp_dir.path().join(".config/jstz/config.json");
    create_dir_all(path.parent().expect("should find parent dir"))
        .expect("should create dir");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(
        file,
        &serde_json::json!({
            "networks": {
                network_name: {
                    "octez_node_rpc_endpoint": "http://octez.test",
                    "jstz_node_endpoint": "http://jstz.test"
                },
            }
        }),
    )
    .expect("should write config file");

    // update non-existent network
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "update",
            "some-random-name",
            "--jstz-node-endpoint",
            "http://v2.jstz.test",
        ])
        .assert()
        .stderr(predicates::str::contains(
            "Network 'some-random-name' does not exist.",
        ))
        .failure();

    // network should be listed
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(network_name)
                .and(predicates::str::contains("http://octez.test"))
                .and(predicates::str::contains("http://jstz.test")),
        )
        .success();

    // update octez endpoint
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "update",
            network_name,
            "--octez-node-rpc-endpoint",
            "http://v2.octez.test",
        ])
        .assert()
        .stderr(predicates::str::contains(format!(
            "Updated network '{network_name}'.",
        )))
        .success();

    // network should be listed with new octez endpoint
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(network_name)
                .and(predicates::str::contains("http://v2.octez.test"))
                .and(predicates::str::contains("http://jstz.test")),
        )
        .success();

    // update jstz endpoint
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "update",
            network_name,
            "--jstz-node-endpoint",
            "http://v2.jstz.test",
        ])
        .assert()
        .stderr(predicates::str::contains(format!(
            "Updated network '{network_name}'.",
        )))
        .success();

    // network should be listed with new jstz endpoint
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(network_name)
                .and(predicates::str::contains("http://v2.octez.test"))
                .and(predicates::str::contains("http://v2.jstz.test")),
        )
        .success();

    // update both
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "update",
            network_name,
            "--jstz-node-endpoint",
            "http://v3.jstz.test",
            "--octez-node-rpc-endpoint",
            "http://v3.octez.test",
        ])
        .assert()
        .stderr(predicates::str::contains(format!(
            "Updated network '{network_name}'.",
        )))
        .success();

    // network should be listed with new jstz endpoint
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "list"])
        .assert()
        .stderr(
            predicates::str::contains(network_name)
                .and(predicates::str::contains("http://v3.octez.test"))
                .and(predicates::str::contains("http://v3.jstz.test")),
        )
        .success();

    // missing options
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args(["network", "update", network_name])
        .assert()
        .stderr(predicates::str::contains(
            "the following required arguments were not provided",
        ))
        .failure();

    // option used multiple times
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", &home_path)
        .args([
            "network",
            "update",
            network_name,
            "--octez-node-rpc-endpoint",
            "http://v4.octez.test",
            "--octez-node-rpc-endpoint",
            "http://v5.octez.test",
        ])
        .assert()
        .stderr(predicates::str::contains(
            "the argument '--octez-node-rpc-endpoint <OCTEZ_NODE_RPC_ENDPOINT>' cannot be used multiple times",
        ))
        .failure();
}
