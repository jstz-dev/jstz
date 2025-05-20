use jstz_node::config::{JstzNodeConfig, KeyPair};
use jstzd::task::{utils::retry, Task};
use octez::{r#async::endpoint::Endpoint, unused_port};
use tempfile::{NamedTempFile, TempDir};

#[tokio::test(flavor = "multi_thread")]
async fn jstz_node_test() {
    let endpoint = Endpoint::localhost(unused_port());
    let mock_rollup_endpoint = Endpoint::localhost(unused_port());
    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_path_buf();
    let preimages_dir = TempDir::new().unwrap();
    let preimages_dir_path = preimages_dir.path().to_path_buf();
    let jstz_node_config = JstzNodeConfig::new(
        &endpoint,
        &mock_rollup_endpoint,
        &preimages_dir_path,
        &path,
        KeyPair::default(),
        jstz_node::RunMode::Default,
        0,
    );
    let mut jstz_node = jstzd::task::jstz_node::JstzNode::spawn(jstz_node_config)
        .await
        .unwrap();
    let jstz_node_ready =
        retry(10, 1000, || async { jstz_node.health_check().await }).await;
    assert!(jstz_node_ready);
    jstz_node.kill().await.unwrap();
    let is_alive = jstz_node.health_check().await.unwrap();
    assert!(!is_alive);
}
