use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use predicates::prelude::predicate;
use std::{io::Write, process::Command};
use tempfile::NamedTempFile;

#[test]
fn unknown_command() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();

    cmd.arg("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand \'test\'"));
}

#[test]
fn default_config() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();

    cmd.arg("run")
        .assert()
        .success()
        .stdout(predicate::str::contains("ready"));
}

#[test]
fn valid_config_file() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file.write_all(r#"{"protocol":{"bootstrap_accounts":[["edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2","6000000000"]]}}"#.as_bytes()).unwrap();

    cmd.args(["run", &tmp_file.path().to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ready"));
}

#[test]
fn bad_config_file() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file.write_all("{}".as_bytes()).unwrap();

    cmd.args(["run", &tmp_file.path().to_string_lossy()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "should have at least one bootstrap account with at least 6000 tez",
        ));
}
