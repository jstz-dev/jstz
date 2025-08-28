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
        .unwrap()
        .wait()
        .unwrap();
    assert!(child.wait().is_ok());
}

#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[test]
fn valid_config_file() {
    let port = unused_port();
    let mut tmp_file = NamedTempFile::new().unwrap();
    tmp_file
        .write_all(format!(r#"{{"server_port":{port}}}"#).as_bytes())
        .unwrap();

    let handle = thread::spawn(move || {
        Command::cargo_bin("jstzd")
            .unwrap()
            .args(["run", &tmp_file.path().to_string_lossy()])
            .assert()
            .success();
    });

    let client = reqwest::blocking::Client::new();
    for _ in 0..30 {
        thread::sleep(Duration::from_secs(1));
        if let Ok(r) = client.get(format!("http://localhost:{port}/health")).send() {
            if r.status().is_success() {
                break;
            }
        }
    }

    // wait for 5 more seconds to ensure that the baker starts baking in order to
    // observe the expected log line above
    thread::sleep(Duration::from_secs(5));
    assert!(client
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
        .unwrap()
        .wait()
        .unwrap();
    assert!(child.wait().is_ok());
}

#[cfg(feature = "v2_runtime")]
#[cfg_attr(feature = "skip-rollup-tests", ignore)]
#[test]
fn test_oracle_config_from_file() {
    let port = unused_port();
    let mut tmp_file = NamedTempFile::new().unwrap();

    // Create a config file with oracle key pair
    let config_json = format!(
        r#"{{
            "server_port": {port},
            "jstz_node": {{
                "endpoint": "http://localhost:8932",
                "rollup_endpoint": "http://localhost:8933",
                "rollup_preimages_dir": "/tmp/preimages",
                "kernel_log_file": "/tmp/kernel.log",
                "mode": "default",
                "capacity": 0,
                "debug_log_file": "/tmp/debug.log",
                "oracle_key_pair": ["edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi", "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM"]
            }}
        }}"#
    );

    tmp_file.write_all(config_json.as_bytes()).unwrap();

    let handle = thread::spawn(move || {
        Command::cargo_bin("jstzd")
            .unwrap()
            .args(["run", &tmp_file.path().to_string_lossy()])
            .assert()
            .success();
    });

    let client = reqwest::blocking::Client::new();
    for _ in 0..30 {
        thread::sleep(Duration::from_secs(1));
        if let Ok(r) = client.get(format!("http://localhost:{port}/health")).send() {
            if r.status().is_success() {
                break;
            }
        }
    }

    // Wait a bit more to ensure oracle node has time to start
    thread::sleep(Duration::from_secs(3));

    // Verify the server is still running
    let health_response = client
        .get(format!("http://localhost:{port}/health"))
        .send()
        .unwrap();
    assert!(health_response.status().is_success());

    // Shutdown
    assert!(client
        .put(format!("http://localhost:{port}/shutdown"))
        .send()
        .unwrap()
        .status()
        .is_success());

    handle
        .join()
        .expect("jstzd should have been taken down without any error");
}
