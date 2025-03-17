mod utils;
use jstzd::task::{utils::get_block_level, Task};
use utils::setup;

#[tokio::test(flavor = "multi_thread")]
async fn test_baker() {
    let (mut octez_node, _, mut baker) = setup(None).await;
    let node_endpoint = octez_node.rpc_endpoint();

    let _ = baker.kill().await;
    assert!(!baker.health_check().await.unwrap());
    // check if the block level stops increasing after killing
    let last_level = get_block_level(&node_endpoint.to_string()).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let current_level = get_block_level(&node_endpoint.to_string()).await.unwrap();
    assert_eq!(last_level, current_level);
    let _ = octez_node.kill().await;
}
