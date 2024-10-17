use super::Task;
use anyhow::Result;
use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use octez::OctezNodeConfig;
use std::{fs::File, sync::Arc};
use tokio::sync::RwLock;

use octez::AsyncOctezNode;
use tokio::process::Child;

#[derive(Default)]
struct ChildWrapper {
    inner: Option<Child>,
}

impl ChildWrapper {
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(mut v) = self.inner.take() {
            v.kill().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncDrop for ChildWrapper {
    async fn async_drop(&mut self) {
        let _ = self.kill().await;
    }
}

#[derive(Default, Clone)]
pub struct OctezNode {
    inner: Arc<RwLock<AsyncDropper<ChildWrapper>>>,
    config: OctezNodeConfig,
}

impl OctezNode {
    pub fn rpc_endpoint(&self) -> &str {
        &self.config.rpc_endpoint
    }
}

#[async_trait]
impl Task for OctezNode {
    type Config = OctezNodeConfig;

    /// Spins up the task with the given config.
    async fn spawn(config: Self::Config) -> Result<Self> {
        let node = AsyncOctezNode {
            octez_node_bin: Some(config.binary_path.clone()),
            octez_node_dir: config.data_dir.clone(),
        };

        let status = node.generate_identity().await?.wait().await?;
        match status.code() {
            Some(0) => (),
            _ => return Err(anyhow::anyhow!("failed to generate node identity")),
        }

        let status = node
            .config_init(
                &config.network,
                &config.rpc_endpoint,
                &config.p2p_address,
                0,
            )
            .await?
            .wait()
            .await?;
        match status.code() {
            Some(0) => (),
            _ => return Err(anyhow::anyhow!("failed to initialise node config")),
        }

        Ok(OctezNode {
            inner: Arc::new(RwLock::new(AsyncDropper::new(ChildWrapper {
                inner: Some(
                    node.run(&File::create(&config.log_file)?, &config.run_options)
                        .await?,
                ),
            }))),
            config,
        })
    }

    /// Aborts the running task.
    async fn kill(&mut self) -> Result<()> {
        let mut inner = self.inner.write().await;
        Ok(inner.inner_mut().kill().await?)
    }

    /// Conducts a health check on the running task.
    async fn health_check(&self) -> Result<bool> {
        // Returns whether or not the node is ready to answer to requests.
        // https://gitlab.com/tezos/tezos/-/raw/2e84c439c25c4d9b363127a6685868e223877034/docs/api/rpc-openapi.json
        let res =
            reqwest::get(format!("http://{}/health/ready", &self.config.rpc_endpoint))
                .await;
        if res.is_err() {
            return Ok(false);
        }
        let body = res
            .unwrap()
            .json::<std::collections::HashMap<String, bool>>()
            .await?;
        if let Some(v) = body.get("ready") {
            return Ok(*v);
        }
        return Err(anyhow::anyhow!("unexpected error: `ready` cannot be retrieved from octez-node health check endpoint"));
    }
}
