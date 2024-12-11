use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use octez::unused_port;
use predicates::prelude::predicate;
use std::thread;
use std::time::Duration;
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

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[test]
fn default_config() {
    // Since the server's port number is unknown when jstzd runs on default config,
    // here it's assumed that if the child process is still alive after 10 seconds,
    // it means that jstzd successfully launched
    let mut child = Command::cargo_bin("jstzd")
        .unwrap()
        .arg("run")
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_secs(10));
    assert!(child.try_wait().unwrap().is_none());
    Command::new("kill")
        .args(["-s", "TERM", &child.id().to_string()])
        .spawn()
        .unwrap();
    assert!(child.wait().is_ok());
}

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[test]
fn valid_config_file() {
    let port = unused_port();
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file.write_all(format!(r#"{{"protocol":{{"bootstrap_accounts":[["edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2","15000000000"]]}},"server_port":{}}}"#, port).as_bytes()).unwrap();

    let handle = thread::spawn(move || {
        Command::cargo_bin("jstzd")
            .unwrap()
            .args(["run", &tmp_file.path().to_string_lossy()])
            .assert()
            .success()
            // baker log writes to stderr
            .stderr(predicate::str::contains(
                "block ready for delegate: activator",
            ));
    });

    let client = reqwest::blocking::Client::new();
    for _ in 0..30 {
        thread::sleep(Duration::from_secs(1));
        if let Ok(r) = client
            .get(&format!("http://localhost:{port}/health"))
            .send()
        {
            if r.status().is_success() {
                break;
            }
        }
    }

    // wait for 5 more seconds to ensure that the baker starts baking in order to
    // observe the expected log line above
    thread::sleep(Duration::from_secs(5));
    assert!(client
        .put(&format!("http://localhost:{port}/shutdown"))
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
    tmp_file.write_all("{}".as_bytes()).unwrap();

    cmd.args(["run", &tmp_file.path().to_string_lossy()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "should have at least one bootstrap account with at least 6000 tez",
        ));
}

#[test]
fn terminate_with_sigint() {
    // Since the server's port number is unknown when jstzd runs on default config,
    // here it's assumed that if the child process is still alive after 10 seconds,
    // it means that jstzd successfully launched
    let mut child = Command::cargo_bin("jstzd")
        .unwrap()
        .arg("run")
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
