mod utils;
use octez::r#async::endpoint::Endpoint;
use octez::r#async::node_config::OctezNodeConfigBuilder;
use octez::unused_port;
use utils::retry;

#[tokio::test(flavor = "multi_thread")]
async fn jstzd_test() {
    let rpc_endpoint = Endpoint::localhost(unused_port());
    let config = jstzd::task::jstzd::JstzdConfig::new(
        OctezNodeConfigBuilder::new()
            .set_rpc_endpoint(&rpc_endpoint)
            .build()
            .unwrap(),
    );
    let jstzd_port = unused_port();
    let mut jstzd = jstzd::task::jstzd::JstzdServer::new(config, jstzd_port);
    jstzd.run().await.unwrap();

    let jstz_health_check_endpoint = format!("http://localhost:{}", jstzd_port);
    let octez_node_health_check_endpoint = format!("{}/health/ready", rpc_endpoint);
    let jstzd_running = retry(10, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
        if res.is_ok() {
            return Ok(true);
        }
        Err(anyhow::anyhow!(""))
    })
    .await;
    assert!(jstzd_running);

    let node_running = retry(10, 1000, || async {
        let res = reqwest::get(&octez_node_health_check_endpoint).await;
        if res.is_ok() {
            return Ok(true);
        }
        Err(anyhow::anyhow!(""))
    })
    .await;
    assert!(node_running);
    assert!(jstzd.health_check().await);

    jstzd.stop().await.unwrap();

    let jstzd_stopped = retry(10, 1000, || async {
        let res = reqwest::get(&jstz_health_check_endpoint).await;
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Err(anyhow::anyhow!(""))
    })
    .await;
    assert!(jstzd_stopped);

    let node_destroyed = retry(10, 1000, || async {
        let res = reqwest::get(&octez_node_health_check_endpoint).await;
        // Should get an error since the node should have been terminated
        if let Err(e) = res {
            return Ok(e.to_string().contains("Connection refused"));
        }
        Err(anyhow::anyhow!(""))
    })
    .await;
    assert!(node_destroyed);

    assert!(!jstzd.health_check().await);
}
