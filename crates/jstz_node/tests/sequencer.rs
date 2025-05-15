use octez::unused_port;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

#[tokio::test]
async fn run_sequencer() {
    let tmp_dir = TempDir::new().unwrap();
    let log_file = NamedTempFile::new().unwrap();
    let port = unused_port();

    let bin_path = assert_cmd::cargo::cargo_bin("jstz-node");
    let mut c = Command::new(bin_path)
        .args([
            "run",
            "--port",
            &port.to_string(),
            "--preimages-dir",
            tmp_dir.path().to_str().unwrap(),
            "--kernel-log-path",
            log_file.path().to_str().unwrap(),
            "--mode",
            "sequencer",
        ])
        .spawn()
        .unwrap();

    let res = jstz_utils::poll(10, 500, || async {
        reqwest::get(format!("http://127.0.0.1:{}/mode", port))
            .await
            .ok()
    })
    .await
    .expect("should get response")
    .text()
    .await
    .expect("should get text body");

    assert_eq!(res, "\"sequencer\"");
    let _ = c.kill();
}
