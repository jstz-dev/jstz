use jstzd::task::{octez_node, Task};
use serial_test::serial;
mod utils;
use utils::retry;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn octez_node_test() {
    let data_dir = tempfile::tempdir().unwrap();
    let log_file = tempfile::NamedTempFile::new().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let rpc_endpoint = format!("localhost:{}", port);

    let mut config_builer = octez_node::OctezNodeConfigBuilder::new();
    config_builer
        .set_binary_path("octez-node")
        .set_data_dir(data_dir.path().to_str().unwrap())
        .set_network("sandbox")
        .set_rpc_endpoint(&rpc_endpoint)
        .set_log_file(log_file.path().to_str().unwrap())
        .set_run_options(&[]);
    let mut f = octez_node::OctezNode::spawn(config_builer.build().unwrap())
        .await
        .unwrap();

    // Should be able to hit the endpoint since the node should have been launched
    let node_ready = retry(10, 1000, || async { f.health_check().await }).await;
    assert!(node_ready);

    let _ = f.kill().await;
    // Wait for the process to shutdown entirely
    let health_check_endpoint = format!("http://{}/health/ready", rpc_endpoint);
    let node_destroyed = retry(10, 1000, || async {
        let res = reqwest::get(&health_check_endpoint).await;
        // Should get an error since the node should have been terminated
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Err(anyhow::anyhow!(""))
    })
    .await;
    assert!(node_destroyed);
}
