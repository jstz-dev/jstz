use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use std::{
    fs::{create_dir_all, File},
    process::Command,
};
use tempfile::TempDir;

fn config() -> serde_json::Value {
    serde_json::json!({
        "current_alias": "foo",
        "accounts": {
            "foo": {
                "User": {
                    "address": "tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6",
                    "secret_key": "edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf",
                    "public_key": "edpktpcAZ3d8Yy1EZUF1yX4xFgLq5sJ7cL9aVhp7aV12y89RXThE3N"
                }
            },
        }
    })
}

#[test]
fn jstz_home_dir() {
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join(".config/jstz/config.json");
    create_dir_all(path.parent().expect("should find parent dir"))
        .expect("should create dir");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(file, &config()).expect("should write config file");

    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", tmp_dir.path().to_string_lossy().to_string())
        .arg("whoami")
        .assert()
        .stderr(predicates::str::contains(
            "Logged in to account foo with address tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6",
        ))
        .success();

    let another_dir = TempDir::new().unwrap();
    // config file does not exist in this new directory
    Command::cargo_bin("jstz")
        .unwrap()
        .env("HOME", another_dir.path().to_string_lossy().to_string())
        .arg("whoami")
        .assert()
        .stderr(predicates::str::contains("You are not logged in"))
        .failure();
}

#[test]
fn xdg_config_home() {
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("config.json");
    let file = File::create(&path).expect("should create file");
    serde_json::to_writer(file, &config()).expect("should write config file");

    Command::cargo_bin("jstz")
        .unwrap()
        .env(
            "XDG_CONFIG_HOME",
            tmp_dir.path().to_string_lossy().to_string(),
        )
        .arg("whoami")
        .assert()
        .stderr(predicates::str::contains(
            "Logged in to account foo with address tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6",
        ))
        .success();

    let another_dir = TempDir::new().unwrap();
    // config file does not exist in this new directory
    Command::cargo_bin("jstz")
        .unwrap()
        .env(
            "XDG_CONFIG_HOME",
            another_dir.path().to_string_lossy().to_string(),
        )
        .arg("whoami")
        .assert()
        .stderr(predicates::str::contains("You are not logged in"))
        .failure();
}
