use super::{
    octez_baker::OctezBaker,
    octez_node::OctezNode,
    octez_rollup::OctezRollup,
    utils::{get_block_level, retry},
    Task,
};
use anyhow::{bail, Context, Result};
use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, put},
    Router,
};
use octez::r#async::{
    baker::OctezBakerConfig,
    client::{OctezClient, OctezClientConfig},
    endpoint::Endpoint,
    node_config::OctezNodeConfig,
    protocol::ProtocolParameter,
    rollup::OctezRollupConfig,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    sync::{oneshot, RwLock},
    task::JoinHandle,
};

pub struct Jstzd {
    octez_node: Arc<RwLock<OctezNode>>,
    baker: Arc<RwLock<OctezBaker>>,
    rollup: Arc<RwLock<OctezRollup>>,
}

#[derive(Clone, Serialize)]
pub struct JstzdConfig {
    #[serde(rename(serialize = "octez-node"))]
    octez_node_config: OctezNodeConfig,
    #[serde(rename(serialize = "octez-baker"))]
    baker_config: OctezBakerConfig,
    #[serde(rename(serialize = "octez-client"))]
    octez_client_config: OctezClientConfig,
    #[serde(skip_serializing)]
    octez_rollup_config: OctezRollupConfig,
    #[serde(skip_serializing)]
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

    pub fn octez_node_config(&self) -> &OctezNodeConfig {
        &self.octez_node_config
    }

    pub fn octez_client_config(&self) -> &OctezClientConfig {
        &self.octez_client_config
    }

    pub fn baker_config(&self) -> &OctezBakerConfig {
        &self.baker_config
    }

    pub fn protocol_params(&self) -> &ProtocolParameter {
        &self.protocol_params
    }
}

#[async_trait]
impl Task for Jstzd {
    type Config = JstzdConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        let octez_node = OctezNode::spawn(config.octez_node_config.clone()).await?;
        let octez_client = OctezClient::new(config.octez_client_config.clone());
        Self::wait_for_node(&octez_node).await?;

        Self::import_activator(&octez_client).await?;
        Self::import_rollup_operator(&octez_client).await?;
        Self::activate_protocol(&octez_client, &config.protocol_params).await?;
        let baker = OctezBaker::spawn(config.baker_config.clone()).await?;
        Self::wait_for_block_level(&config.octez_node_config.rpc_endpoint, 3).await?;
        let rollup = OctezRollup::spawn(config.octez_rollup_config.clone()).await?;
        Ok(Self {
            octez_node: Arc::new(RwLock::new(octez_node)),
            baker: Arc::new(RwLock::new(baker)),
            rollup: Arc::new(RwLock::new(rollup)),
        })
    }

    async fn kill(&mut self) -> Result<()> {
        let results = futures::future::join_all([
            self.octez_node.write().await.kill(),
            self.baker.write().await.kill(),
            self.rollup.write().await.kill(),
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
            self.rollup.read().await.health_check(),
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
    pub const ROLLUP_OPERATOR_ACCOUNT_ALIAS: &'static str = "bootstrap1";

    async fn import_activator(octez_client: &OctezClient) -> Result<()> {
        octez_client
            .import_secret_key(Self::ACTIVATOR_ACCOUNT_ALIAS, Self::ACTIVATOR_ACCOUNT_SK)
            .await
            .context("Failed to import account 'activator'")
    }

    async fn import_rollup_operator(octez_client: &OctezClient) -> Result<()> {
        octez_client
            .import_secret_key(
                Self::ROLLUP_OPERATOR_ACCOUNT_ALIAS,
                Self::ROLLUP_OPERATOR_ACCOUNT_SK,
            )
            .await
            .context("Failed to import account 'rollup_operator'")
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
            get_block_level(&node_endpoint.to_string())
                .await
                .map(|l| l >= level)
        })
        .await;
        if !ready {
            bail!("baker is not ready after retries");
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct JstzdServerInner {
    state: Arc<RwLock<ServerState>>,
}

#[derive(Default)]
struct ServerState {
    jstzd_config: Option<JstzdConfig>,
    jstzd_config_json: serde_json::Map<String, serde_json::Value>,
    jstzd: Option<Jstzd>,
    server_handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

#[async_trait]
impl AsyncDrop for JstzdServerInner {
    async fn async_drop(&mut self) {
        let mut lock = self.state.write().await;
        let _ = shutdown(&mut lock).await;
    }
}

pub struct JstzdServer {
    port: u16,
    inner: Arc<AsyncDropper<JstzdServerInner>>,
    shutdown_rx: Option<oneshot::Receiver<()>>,
}

impl JstzdServer {
    pub fn new(config: JstzdConfig, port: u16) -> Self {
        Self {
            port,
            inner: Arc::new(AsyncDropper::new(JstzdServerInner {
                state: Arc::new(RwLock::new(ServerState {
                    jstzd_config_json: serde_json::to_value(&config)
                        .unwrap()
                        .as_object()
                        .unwrap()
                        .to_owned(),
                    jstzd_config: Some(config),
                    jstzd: None,
                    server_handle: None,
                    shutdown_tx: None,
                })),
            })),
            shutdown_rx: None,
        }
    }

    pub async fn wait(&mut self) {
        if let Some(rx) = self.shutdown_rx.take() {
            let _ = rx.await;
        }
    }

    pub async fn health_check(&self) -> bool {
        let lock = self.inner.state.read().await;
        health_check(&lock).await
    }

    pub async fn stop(&mut self) -> Result<()> {
        let mut lock = self.inner.state.write().await;
        shutdown(&mut lock).await
    }

    pub async fn run(&mut self) -> Result<()> {
        let jstzd = Jstzd::spawn(
            self.inner
                .state
                .read()
                .await
                .jstzd_config
                .as_ref()
                .ok_or(anyhow::anyhow!(
                    // shouldn't really reach this branch since jstzd config is required at instantiation
                    // unless someone calls `run` after calling `stop`
                    "cannot run jstzd server without jstzd config"
                ))?
                .clone(),
        )
        .await?;
        self.inner.state.write().await.jstzd.replace(jstzd);

        let router = Router::new()
            .route("/health", get(health_check_handler))
            .route("/shutdown", put(shutdown_handler))
            .route("/config/:config_type", get(config_handler))
            .route("/config/", get(all_config_handler))
            .with_state(self.inner.state.clone());
        let listener = TcpListener::bind(("0.0.0.0", self.port)).await?;

        let handle = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });
        self.inner.state.write().await.server_handle.replace(handle);
        let (tx, rx) = oneshot::channel();
        self.shutdown_rx.replace(rx);
        self.inner.state.write().await.shutdown_tx.replace(tx);
        Ok(())
    }

    pub async fn baker_healthy(&self) -> bool {
        if let Some(v) = &self.inner.state.read().await.jstzd {
            v.baker.read().await.health_check().await.unwrap_or(false)
        } else {
            false
        }
    }

    pub async fn rollup_healthy(&self) -> bool {
        match &self.inner.state.read().await.jstzd {
            Some(v) => v.rollup.read().await.health_check().await.unwrap_or(false),
            None => false,
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
    state.jstzd_config.take();
    state.jstzd_config_json.clear();
    if let Some(v) = state.shutdown_tx.take() {
        let _ = v.send(());
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

async fn all_config_handler(state: State<Arc<RwLock<ServerState>>>) -> impl IntoResponse {
    let config = &state.read().await.jstzd_config_json;
    serde_json::to_string(config).unwrap().into_response()
}

async fn config_handler(
    state: State<Arc<RwLock<ServerState>>>,
    Path(config_type): Path<String>,
) -> impl IntoResponse {
    let config = &state.read().await.jstzd_config_json;
    match config.get(&config_type) {
        Some(v) => match serde_json::to_string(v) {
            Ok(s) => s.into_response(),
            // TODO: log this error
            Err(_) => http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        None => http::StatusCode::NOT_FOUND.into_response(),
    }
}
