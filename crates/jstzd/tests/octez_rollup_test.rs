mod utils;
use jstzd::task::Task;
use std::path::Path;
use utils::{setup, spawn_rollup};

#[tokio::test(flavor = "multi_thread")]
async fn test_rollup() {
    let (mut octez_node, client, mut baker) = setup(None).await;
    let kernel_installer = Path::new(std::env!("CARGO_MANIFEST_DIR"))
        .join("tests/toy_rollup/kernel_installer");
    let preimages_dir =
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/toy_rollup/preimages");
    let mut rollup =
        spawn_rollup(&octez_node, &client, kernel_installer, preimages_dir, None).await;
    let _ = rollup.kill().await;
    // Should get an error since the rollup node should have been terminated
    let rollup_is_alive = rollup.health_check().await;
    assert!(
        rollup_is_alive.is_err_and(|e| e.to_string().contains("error trying to connect")),
    );
    let _ = baker.kill().await;
    let _ = octez_node.kill().await;
}
