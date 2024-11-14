use super::{
    octez_baker::OctezBaker,
    octez_node::OctezNode,
    octez_rollup::OctezRollup,
    utils::{get_block_level, retry},
    Task,
};
use anyhow::{anyhow, Result};
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use axum::Router;
use octez::r#async::{
    baker::OctezBakerConfig,
    client::{OctezClient, OctezClientConfig},
    endpoint::Endpoint,
    node_config::OctezNodeConfig,
    protocol::ProtocolParameter,
    rollup::OctezRollupConfig,
};
use tokio::{net::TcpListener, task::JoinHandle};

struct Jstzd {
    octez_node: OctezNode,
    baker: OctezBaker,
    rollup: OctezRollup,
}

#[derive(Clone)]
pub struct JstzdConfig {
    octez_node_config: OctezNodeConfig,
    baker_config: OctezBakerConfig,
    octez_client_config: OctezClientConfig,
    octez_rollup_config: OctezRollupConfig,
    protocol_params: ProtocolParameter,
}

impl JstzdConfig {
    pub fn new(
        octez_node_config: OctezNodeConfig,
        baker_config: OctezBakerConfig,
        octez_client_config: OctezClientConfig,
        octez_rollup_config: OctezRollupConfig,
        protocol_params: ProtocolParameter,
    ) -> Self {
        Self {
            octez_node_config,
            baker_config,
            octez_client_config,
            octez_rollup_config,
            protocol_params,
        }
    }
}

#[async_trait]
impl Task for Jstzd {
    type Config = JstzdConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let octez_node = OctezNode::spawn(config.octez_node_config.clone()).await?;
        let octez_client = OctezClient::new(config.octez_client_config.clone());
        Self::wait_for_node(&octez_node).await?;

        Self::import_activator(&octez_client).await;
        Self::import_rollup_operator(&octez_client).await;
        Self::activate_protocol(&octez_client, &config.protocol_params).await?;
        let baker = OctezBaker::spawn(config.baker_config.clone()).await?;
        Self::wait_for_block_level(&config.octez_node_config.rpc_endpoint, 3).await?;
        let rollup = OctezRollup::spawn(config.octez_rollup_config.clone()).await?;
        Ok(Self {
            octez_node,
            baker,
            rollup,
        })
    }

    async fn kill(&mut self) -> Result<()> {
        let results = futures::future::join_all([
            self.octez_node.kill(),
            self.baker.kill(),
            self.rollup.kill(),
        ])
        .await;

        let mut err = vec![];
        for result in results {
            if let Err(e) = result {
                err.push(e);
            }
        }

        if !err.is_empty() {
            Err(anyhow::anyhow!("failed to kill jstzd: {:?}", err))
        } else {
            Ok(())
        }
    }

    async fn health_check(&self) -> Result<bool> {
        let check_results = futures::future::join_all([
            self.octez_node.health_check(),
            self.baker.health_check(),
            self.rollup.health_check(),
        ])
        .await;

        let mut healthy = true;
        let mut err = vec![];
        for result in check_results {
            match result {
                Err(e) => err.push(e),
                Ok(v) => healthy = healthy && v,
            }
        }

        if !err.is_empty() {
            Err(anyhow::anyhow!("failed to perform health check: {:?}", err))
        } else {
            Ok(healthy)
        }
    }
}

impl Jstzd {
    const ACTIVATOR_ACCOUNT_SK: &'static str =
        "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";
    const ACTIVATOR_ACCOUNT_ALIAS: &'static str = "activator";
    const ROLLUP_OPERATOR_ACCOUNT_SK: &'static str =
        "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh";
    const ROLLUP_OPERATOR_ACCOUNT_ALIAS: &'static str = "bootstrap1";

    async fn import_activator(octez_client: &OctezClient) {
        octez_client
            .import_secret_key(Self::ACTIVATOR_ACCOUNT_ALIAS, Self::ACTIVATOR_ACCOUNT_SK)
            .await
            .expect("Failed to import account 'activator'");
    }

    async fn import_rollup_operator(octez_client: &OctezClient) {
        octez_client
            .import_secret_key(
                Self::ROLLUP_OPERATOR_ACCOUNT_ALIAS,
                Self::ROLLUP_OPERATOR_ACCOUNT_SK,
            )
            .await
            .expect("Failed to import account 'rollup_operator'");
    }

    async fn activate_protocol(
        octez_client: &OctezClient,
        protocol_params: &ProtocolParameter,
    ) -> Result<()> {
        octez_client
            .activate_protocol(
                protocol_params.protocol().hash(),
                "0",
                "activator",
                protocol_params.parameter_file().path(),
            )
            .await
    }

    async fn wait_for_node(octez_node: &OctezNode) -> Result<()> {
        let ready = retry(10, 1000, || async {
            Ok(octez_node.health_check().await.unwrap_or(false))
        })
        .await;
        if !ready {
            return Err(anyhow::anyhow!(
                "octez node is still not ready after retries"
            ));
        }
        Ok(())
    }

    /// Wait for the baker to bake at least `level` blocks.
    async fn wait_for_block_level(node_endpoint: &Endpoint, level: i64) -> Result<()> {
        let ready = retry(10, 1000, || async {
            Ok(get_block_level(&node_endpoint.to_string()).await? >= level)
        })
        .await;
        if !ready {
            return Err(anyhow!("baker is not ready after retries"));
        }
        Ok(())
    }
}

pub struct JstzdServer {
    jstzd: Option<Jstzd>,
    jstzd_config: JstzdConfig,
    jstzd_server_port: u16,
    server_handle: Option<JoinHandle<()>>,
}

#[async_trait]
impl AsyncDrop for JstzdServer {
    async fn async_drop(&mut self) {
        if let Err(e) = self.stop().await {
            eprintln!("failed to stop jstzd server: {:?}", e);
        }
    }
}

impl JstzdServer {
    pub fn new(config: JstzdConfig, port: u16) -> Self {
        Self {
            jstzd: None,
            jstzd_config: config,
            jstzd_server_port: port,
            server_handle: None,
        }
    }

    pub async fn health_check(&self) -> bool {
        if let Some(v) = &self.server_handle {
            if !v.is_finished() {
                if let Some(jstzd) = &self.jstzd {
                    if let Ok(v) = jstzd.health_check().await {
                        return v;
                    }
                }
            }
        }

        false
    }

    pub async fn stop(&mut self) -> Result<()> {
        let mut err = None;
        if let Some(mut jstzd) = self.jstzd.take() {
            if let Err(e) = jstzd.kill().await {
                err.replace(e);
            };
        }
        if let Some(server) = self.server_handle.take() {
            server.abort();
        }
        match err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.jstzd
            .replace(Jstzd::spawn(self.jstzd_config.clone()).await?);

        let router = Router::new().route("/", axum::routing::get(http::StatusCode::OK));
        let listener = TcpListener::bind(("0.0.0.0", self.jstzd_server_port)).await?;

        let handle = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });
        self.server_handle.replace(handle);
        Ok(())
    }
}
