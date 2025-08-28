use anyhow::Result;
use async_trait::async_trait;
use jstz_oracle_node::{node::OracleNode as InnerOracleNode, OracleNodeConfig};
use jstz_utils::KeyPair;

use crate::task::Task;

pub struct OracleNode {
    inner: Option<InnerOracleNode>,
}

#[async_trait]
impl Task for OracleNode {
    type Config = OracleNodeConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let oracle = if let Some(key_pair) = &config.key_pair {
            let KeyPair(public_key, secret_key) = key_pair;
            Some(
                InnerOracleNode::spawn(
                    config.log_path.clone(),
                    public_key.clone(),
                    secret_key.clone(),
                    config.jstz_node_endpoint.to_string(),
                )
                .await?,
            )
        } else {
            None
        };
        Ok(Self { inner: oracle })
    }

    async fn kill(&mut self) -> Result<()> {
        drop(self.inner.take());
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // TODO: implement this properly once oracle node exposes health information
        Ok(true)
    }
}
