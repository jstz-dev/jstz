use super::{octez_baker::OctezBaker, octez_node::OctezNode, utils::retry, Task};
use anyhow::Result;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use axum::{
    extract::State,
    routing::{get, put},
    Router,
};
use octez::r#async::{
    baker::OctezBakerConfig,
    client::{OctezClient, OctezClientConfig},
    node_config::OctezNodeConfig,
    protocol::ProtocolParameter,
};
use std::sync::Arc;
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle};

#[derive(Clone)]
struct Jstzd {
    octez_node: Arc<RwLock<OctezNode>>,
    baker: Arc<RwLock<OctezBaker>>,
}

#[derive(Clone)]
pub struct JstzdConfig {
    octez_node_config: OctezNodeConfig,
    baker_config: OctezBakerConfig,
    octez_client_config: OctezClientConfig,
    protocol_params: ProtocolParameter,
}

impl JstzdConfig {
    pub fn new(
        octez_node_config: OctezNodeConfig,
        baker_config: OctezBakerConfig,
        octez_client_config: OctezClientConfig,
        protocol_params: ProtocolParameter,
    ) -> Self {
        Self {
            octez_node_config,
            baker_config,
            octez_client_config,
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
        Self::activate_protocol(&octez_client, &config.protocol_params).await?;

        let baker = OctezBaker::spawn(config.baker_config.clone()).await?;
        Ok(Self {
            octez_node: Arc::new(RwLock::new(octez_node)),
            baker: Arc::new(RwLock::new(baker)),
        })
    }

    async fn kill(&mut self) -> Result<()> {
        let results = futures::future::join_all([
            self.octez_node.write().await.kill(),
            self.baker.write().await.kill(),
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
            self.octez_node.read().await.health_check(),
            self.baker.read().await.health_check(),
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

    async fn import_activator(octez_client: &OctezClient) {
        octez_client
            .import_secret_key(Self::ACTIVATOR_ACCOUNT_ALIAS, Self::ACTIVATOR_ACCOUNT_SK)
            .await
            .expect("Failed to import account 'activator'");
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
}

pub struct JstzdServer {
    jstzd_config: JstzdConfig,
    jstzd_server_port: u16,
    state: Arc<RwLock<ServerState>>,
}

struct ServerState {
    jstzd: Option<Jstzd>,
    server_handle: Option<JoinHandle<()>>,
}

#[async_trait]
impl AsyncDrop for JstzdServer {
    async fn async_drop(&mut self) {
        let mut lock = self.state.write().await;
        let _ = shutdown(&mut lock).await;
    }
}

impl JstzdServer {
    pub fn new(config: JstzdConfig, port: u16) -> Self {
        Self {
            jstzd_config: config,
            jstzd_server_port: port,
            state: Arc::new(RwLock::new(ServerState {
                jstzd: None,
                server_handle: None,
            })),
        }
    }

    pub async fn health_check(&self) -> bool {
        let lock = self.state.read().await;
        health_check(&lock).await
    }

    pub async fn stop(&mut self) -> Result<()> {
        let mut lock = self.state.write().await;
        shutdown(&mut lock).await
    }

    pub async fn run(&mut self) -> Result<()> {
        let jstzd = Jstzd::spawn(self.jstzd_config.clone()).await?;
        self.state.write().await.jstzd.replace(jstzd);

        let router = Router::new()
            .route("/health", get(health_check_handler))
            .route("/shutdown", put(shutdown_handler))
            .with_state(self.state.clone());
        let listener = TcpListener::bind(("0.0.0.0", self.jstzd_server_port)).await?;

        let handle = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });
        self.state.write().await.server_handle.replace(handle);
        Ok(())
    }

    pub async fn baker_healthy(&self) -> bool {
        if let Some(v) = &self.state.read().await.jstzd {
            v.baker.read().await.health_check().await.unwrap_or(false)
        } else {
            false
        }
    }
}

async fn health_check(state: &ServerState) -> bool {
    if let Some(v) = &state.server_handle {
        if !v.is_finished() {
            if let Some(jstzd) = &state.jstzd {
                if let Ok(v) = jstzd.health_check().await {
                    return v;
                }
            }
        }
    }
    false
}

async fn shutdown(state: &mut ServerState) -> Result<()> {
    if let Some(mut jstzd) = state.jstzd.take() {
        if let Err(e) = jstzd.kill().await {
            eprintln!("failed to shutdown jstzd: {:?}", e);
            state.jstzd.replace(jstzd);
            return Err(e);
        };
    }
    if let Some(server) = state.server_handle.take() {
        server.abort();
    }
    Ok(())
}

async fn health_check_handler(
    state: State<Arc<RwLock<ServerState>>>,
) -> http::StatusCode {
    let lock = state.read().await;
    match health_check(&lock).await {
        true => http::StatusCode::OK,
        _ => http::StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn shutdown_handler(state: State<Arc<RwLock<ServerState>>>) -> http::StatusCode {
    let mut lock = state.write().await;
    if shutdown(&mut lock).await.is_err() {
        return http::StatusCode::INTERNAL_SERVER_ERROR;
    };
    http::StatusCode::NO_CONTENT
}
