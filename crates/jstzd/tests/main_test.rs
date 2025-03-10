use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use octez::r#async::endpoint::Endpoint;
use octez::unused_port;
use predicates::prelude::predicate;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::{io::Write, process::Command};
use tempfile::NamedTempFile;

fn create_config_file(port: u16) -> NamedTempFile {
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file
        .write_all(
            format!(
                r#"{{"server_port":{}, "octez_rollup":{{"endpoint":"{}"}}}}"#,
                port,
                Endpoint::localhost(unused_port()).to_string()
            )
            .as_bytes(),
        )
        .unwrap();
    tmp_file
}

fn run_jstzd(port: u16, config_path: Option<&str>) -> anyhow::Result<JoinHandle<()>> {
    let mut args = vec!["run".to_string()];
    if let Some(v) = config_path {
        args.push(v.to_owned());
    }

    let handle = thread::spawn(move || {
        Command::cargo_bin("jstzd")
            .unwrap()
            .args(args)
            .assert()
            .success();
    });

    let client = reqwest::blocking::Client::new();
    for _ in 0..30 {
        thread::sleep(Duration::from_secs(1));
        if let Ok(r) = client.get(format!("http://localhost:{port}/health")).send() {
            if r.status().is_success() {
                return Ok(handle);
            }
        }
    }
    anyhow::bail!("failed")
}

#[test]
fn unknown_command() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();

    cmd.arg("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand \'test\'"));
}

#[test]
fn valid_config_file() {
    let port = unused_port();
    let tmp_file = create_config_file(port);
    let handle = run_jstzd(port, Some(&tmp_file.path().to_string_lossy())).unwrap();
    // wait for 5 more seconds to ensure that the baker starts baking in order to
    // observe the expected log line above
    thread::sleep(Duration::from_secs(5));
    assert!(reqwest::blocking::Client::new()
        .put(format!("http://localhost:{port}/shutdown"))
        .send()
        .unwrap()
        .status()
        .is_success());
    handle
        .join()
        .expect("jstzd should have been taken down without any error");
}

#[test]
fn bad_config_file() {
    let mut cmd = Command::cargo_bin("jstzd").unwrap();
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file
        .write_all("{\"protocol\":{\"protocol\":\"foo\"}}".as_bytes())
        .unwrap();

    cmd.args(["run", &tmp_file.path().to_string_lossy()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to build config"));
}

#[test]
fn terminate_with_sigint() {
    let port = unused_port();
    let tmp_file = create_config_file(port);
    let mut child = Command::cargo_bin("jstzd")
        .unwrap()
        .args(["run", &tmp_file.path().to_string_lossy()])
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_secs(10));
    assert!(child.try_wait().unwrap().is_none());
    Command::new("kill")
        .args(["-s", "INT", &child.id().to_string()])
        .spawn()
        .unwrap();
    assert!(child.wait().is_ok());
}
