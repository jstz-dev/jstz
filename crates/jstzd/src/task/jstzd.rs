use crate::config::BOOTSTRAP_ACCOUNTS;

use super::{
    child_wrapper::Shared,
    jstz_node::JstzNode,
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
    extract::{Json, Path, State},
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use indicatif::{ProgressBar, ProgressStyle};
use jstz_crypto::public_key::PublicKey;
use jstz_node::config::JstzNodeConfig;
use octez::r#async::{
    baker::OctezBakerConfig,
    client::{Address, OctezClient, OctezClientConfig},
    endpoint::Endpoint,
    node_config::OctezNodeConfig,
    protocol::{BootstrapAccount, ProtocolParameter},
    rollup::OctezRollupConfig,
};
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{
    collections::HashMap,
    io::{stdout, Write},
};
use tokio::{
    net::TcpListener,
    sync::{oneshot, RwLock},
    task::JoinHandle,
    time::{sleep, Duration},
};

const ACTIVATOR_ACCOUNT_ALIAS: &str = "bootstrap0";

trait IntoShared {
    fn into_shared(self) -> Shared<Self>;
}

impl<T: Task> IntoShared for T {
    fn into_shared(self) -> Shared<Self> {
        Arc::new(RwLock::new(self))
    }
}

struct Jstzd {
    octez_node: Shared<OctezNode>,
    baker: Shared<OctezBaker>,
    rollup: Shared<OctezRollup>,
    jstz_node: Shared<JstzNode>,
}

#[derive(Clone, Serialize)]
pub struct JstzdConfig {
    #[serde(rename(serialize = "octez_node"))]
    octez_node_config: OctezNodeConfig,
    #[serde(rename(serialize = "octez_baker"))]
    baker_config: OctezBakerConfig,
    #[serde(rename(serialize = "octez_client"))]
    octez_client_config: OctezClientConfig,
    #[serde(rename(serialize = "octez_rollup"))]
    octez_rollup_config: OctezRollupConfig,
    #[serde(rename(serialize = "jstz_node"))]
    jstz_node_config: JstzNodeConfig,
    #[serde(skip_serializing)]
    protocol_params: ProtocolParameter,
}

impl JstzdConfig {
    pub fn new(
        octez_node_config: OctezNodeConfig,
        baker_config: OctezBakerConfig,
        octez_client_config: OctezClientConfig,
        octez_rollup_config: OctezRollupConfig,
        jstz_node_config: JstzNodeConfig,
        protocol_params: ProtocolParameter,
    ) -> Self {
        Self {
            octez_node_config,
            baker_config,
            octez_client_config,
            octez_rollup_config,
            jstz_node_config,
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

    pub fn octez_rollup_config(&self) -> &OctezRollupConfig {
        &self.octez_rollup_config
    }

    pub fn jstz_node_config(&self) -> &JstzNodeConfig {
        &self.jstz_node_config
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

        Self::import_accounts(
            &octez_client,
            HashMap::from_iter(
                // cannot use config.protocol_params().bootstrap_accounts() here because
                // we need secret keys
                BOOTSTRAP_ACCOUNTS
                    .into_iter()
                    .map(|(alias, _, sk)| (alias, sk)),
            ),
        )
        .await?;
        Self::activate_protocol(&octez_client, &config.protocol_params).await?;
        let baker = OctezBaker::spawn(config.baker_config.clone()).await?;
        Self::wait_for_block_level(&config.octez_node_config.rpc_endpoint, 3).await?;
        let rollup = OctezRollup::spawn(config.octez_rollup_config.clone()).await?;
        let jstz_node = JstzNode::spawn(config.jstz_node_config.clone()).await?;
        Ok(Self {
            octez_node: octez_node.into_shared(),
            baker: baker.into_shared(),
            rollup: rollup.into_shared(),
            jstz_node: jstz_node.into_shared(),
        })
    }

    async fn kill(&mut self) -> Result<()> {
        let results = futures::future::join_all([
            self.octez_node.write().await.kill(),
            self.baker.write().await.kill(),
            self.rollup.write().await.kill(),
            self.jstz_node.write().await.kill(),
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
        self.health_check_inner().await.0
    }
}

impl Jstzd {
    async fn health_check_inner(&self) -> (Result<bool>, Vec<Result<bool>>) {
        let check_results = futures::future::join_all([
            self.octez_node.read().await.health_check(),
            self.baker.read().await.health_check(),
            self.rollup.read().await.health_check(),
            self.jstz_node.read().await.health_check(),
        ])
        .await;

        let mut healthy = true;
        let mut err = vec![];
        for result in &check_results {
            match result {
                Err(e) => err.push(e),
                Ok(v) => healthy = healthy && *v,
            }
        }

        if !err.is_empty() {
            (
                Err(anyhow::anyhow!("failed to perform health check: {:?}", err)),
                check_results,
            )
        } else {
            (Ok(healthy), check_results)
        }
    }

    async fn import_accounts(
        octez_client: &OctezClient,
        accounts: HashMap<&str, &str>,
    ) -> Result<()> {
        for (alias, sk) in accounts.iter() {
            octez_client
                .import_secret_key(alias, sk)
                .await
                .context(format!("Failed to import account '{alias}'"))?;
        }
        Ok(())
    }

    async fn activate_protocol(
        octez_client: &OctezClient,
        protocol_params: &ProtocolParameter,
    ) -> Result<()> {
        octez_client
            .activate_protocol(
                protocol_params.protocol().hash(),
                "0",
                ACTIVATOR_ACCOUNT_ALIAS,
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
    state: Shared<ServerState>,
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

    pub async fn run(&mut self, print_info: bool) -> Result<()> {
        let jstzd = Self::spawn_jstzd(
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
            print_info,
        )
        .await?;
        self.inner.state.write().await.jstzd.replace(jstzd);

        let router = Router::new()
            .route("/health", get(health_check_handler))
            .route("/shutdown", put(shutdown_handler))
            .route("/config/:config_type", get(config_handler))
            .route("/config/", get(all_config_handler))
            .route("/contract_call", post(call_contract_handler))
            .route("/l1_alias/:alias", get(l1_alias_handler))
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

    pub async fn jstz_node_healthy(&self) -> bool {
        match &self.inner.state.read().await.jstzd {
            Some(v) => v
                .jstz_node
                .read()
                .await
                .health_check()
                .await
                .unwrap_or(false),
            None => false,
        }
    }

    async fn spawn_jstzd(jstzd_config: JstzdConfig, print_info: bool) -> Result<Jstzd> {
        let mut jstzd = Jstzd::spawn(jstzd_config.clone()).await?;

        let progress_bar = create_progress_bar(print_info);
        let mut jstzd_healthy = false;
        // 60 seconds
        for _ in 0..120 {
            let (overall_result, individual_results) = jstzd.health_check_inner().await;
            jstzd_healthy = overall_result.unwrap_or_default();
            let latest_progress = collect_progress(individual_results);
            update_progress_bar(progress_bar.as_ref(), latest_progress);

            if jstzd_healthy {
                clear_progress_bar(progress_bar.as_ref());
                break;
            }

            sleep(Duration::from_millis(500)).await;
        }

        if !jstzd_healthy {
            let _ = jstzd.kill().await;
            abandon_progress_bar(progress_bar.as_ref());
            bail!("jstzd never turns healthy");
        }

        if print_info {
            print_bootstrap_accounts(
                &mut stdout(),
                jstzd_config.protocol_params().bootstrap_accounts(),
            );
        }

        Ok(jstzd)
    }
}

fn collect_progress(individual_results: Vec<Result<bool>>) -> u64 {
    individual_results
        .into_iter()
        .fold(0, |acc, v| acc + v.unwrap_or_default() as u64)
}

fn create_progress_bar(print_info: bool) -> Option<ProgressBar> {
    match print_info {
        true => {
            let v = ProgressBar::new(4);
            v.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
                    )
                    .unwrap(),
            );
            Some(v)
        }
        false => None,
    }
}

fn clear_progress_bar(progress_bar: Option<&ProgressBar>) {
    if let Some(bar) = progress_bar {
        bar.finish_and_clear();
    }
}

fn update_progress_bar(progress_bar: Option<&ProgressBar>, progress: u64) {
    if let Some(bar) = progress_bar {
        if progress > bar.position() {
            bar.set_position(progress);
        }
    }
}

fn abandon_progress_bar(progress_bar: Option<&ProgressBar>) {
    if let Some(b) = progress_bar {
        b.abandon();
    }
}

fn print_bootstrap_accounts<'a>(
    writer: &mut impl Write,
    accounts: impl IntoIterator<Item = &'a BootstrapAccount>,
) {
    let alias_address_mapping: HashMap<String, &str> = HashMap::from_iter(
        BOOTSTRAP_ACCOUNTS
            .map(|(alias, pk, _)| (PublicKey::from_base58(pk).unwrap().hash(), alias)),
    );

    let mut table = Table::new();
    table.set_titles(Row::new(vec![
        Cell::new("Address"),
        Cell::new("XTZ Balance (mutez)"),
    ]));

    let mut lines = accounts
        .into_iter()
        .map(|account| {
            let address_string = match alias_address_mapping.get(&account.address()) {
                Some(alias) => format!("({alias}) {}", account.address()),
                None => account.address(),
            };
            (address_string, account.amount().to_string())
        })
        .collect::<Vec<_>>();
    lines.sort();
    for (address, amount) in lines {
        table.add_row(Row::new(vec![Cell::new(&address), Cell::new(&amount)]));
    }

    table.set_format({
        let mut format = *FORMAT_DEFAULT;
        format.indent(2);
        format
    });

    let _ = writeln!(writer, "{}", table);
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

async fn health_check_handler(state: State<Shared<ServerState>>) -> http::StatusCode {
    let lock = state.read().await;
    match health_check(&lock).await {
        true => http::StatusCode::OK,
        _ => http::StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn shutdown_handler(state: State<Shared<ServerState>>) -> http::StatusCode {
    let mut lock = state.write().await;
    if shutdown(&mut lock).await.is_err() {
        return http::StatusCode::INTERNAL_SERVER_ERROR;
    };
    http::StatusCode::NO_CONTENT
}

async fn all_config_handler(state: State<Shared<ServerState>>) -> impl IntoResponse {
    let config = &state.read().await.jstzd_config_json;
    serde_json::to_string(config).unwrap().into_response()
}

async fn config_handler(
    state: State<Shared<ServerState>>,
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

async fn l1_alias_handler(
    state: State<Shared<ServerState>>,
    Path(alias): Path<String>,
) -> impl IntoResponse {
    let lock = state.read().await;
    let c = lock.jstzd_config.as_ref().unwrap().octez_client_config();
    handle_show_address_response(
        OctezClient::new(c.clone())
            .show_address(&alias, false)
            .await,
    )
}

// split from l1_alias_handler so that this part can be easily tested
fn handle_show_address_response(res: Result<Address>) -> impl IntoResponse {
    match res {
        Ok(v) => v.hash.to_string().into_response(),
        Err(e) => {
            let s = e.to_string();
            if s.contains("no public key hash alias") {
                http::StatusCode::NOT_FOUND.into_response()
            } else {
                // TODO: log the error
                http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Deserialize)]
struct TransferRequest {
    from: String,
    contract: String,
    amount: f64,
    entrypoint: String,
    arg: String,
}

async fn call_contract_handler(
    state: State<Shared<ServerState>>,
    Json(payload): Json<TransferRequest>,
) -> http::StatusCode {
    let lock = state.read().await;
    let c = lock.jstzd_config.as_ref().unwrap().octez_client_config();
    match OctezClient::new(c.clone())
        .call_contract(
            &payload.from,
            &payload.contract,
            payload.amount,
            &payload.entrypoint,
            &payload.arg,
            Some(100f64),
        )
        .await
    {
        Ok(_) => http::StatusCode::OK,
        _ => http::StatusCode::BAD_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};
    use indicatif::ProgressBar;
    use jstz_crypto::{public_key::PublicKey, public_key_hash::PublicKeyHash};
    use std::path::PathBuf;
    use std::str::FromStr;

    use jstz_node::config::{JstzNodeConfig, KeyPair};
    use octez::r#async::{
        baker::{BakerBinaryPath, OctezBakerConfigBuilder},
        client::{Address, OctezClientConfigBuilder},
        endpoint::Endpoint,
        node_config::OctezNodeConfigBuilder,
        protocol::{BootstrapAccount, ProtocolParameterBuilder},
        rollup::OctezRollupConfigBuilder,
    };
    use tezos_crypto_rs::hash::SmartRollupHash;

    use super::JstzdConfig;

    #[test]
    fn collect_progress() {
        assert_eq!(super::collect_progress(vec![Ok(true), Ok(false)]), 1);
        assert_eq!(
            super::collect_progress(vec![Ok(true), Err(anyhow::anyhow!(""))]),
            1
        );
        assert_eq!(super::collect_progress(vec![Ok(true), Ok(true)]), 2);
        assert_eq!(super::collect_progress(vec![Ok(false), Ok(false)]), 0);
    }

    #[test]
    fn clear_progress_bar() {
        let bar = ProgressBar::new(3);
        super::clear_progress_bar(Some(&bar));
        assert!(bar.is_finished());
    }

    #[test]
    fn update_progress_bar() {
        let bar = ProgressBar::new(3);
        super::update_progress_bar(Some(&bar), 2);
        assert_eq!(bar.position(), 2);

        super::update_progress_bar(Some(&bar), 1);
        assert_eq!(bar.position(), 2);
    }

    #[test]
    fn create_progress_bar() {
        assert!(super::create_progress_bar(false).is_none());

        let bar = super::create_progress_bar(true).unwrap();
        assert_eq!(bar.length().unwrap(), 4);
    }

    #[test]
    fn abandon_progress_bar() {
        let bar = ProgressBar::new(3);
        super::abandon_progress_bar(Some(&bar));
        assert!(bar.is_finished());
    }

    #[test]
    fn print_bootstrap_accounts() {
        let mut buf = vec![];
        super::print_bootstrap_accounts(
            &mut buf,
            [
                &BootstrapAccount::new(
                    "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv",
                    1,
                )
                .unwrap(),
                &BootstrapAccount::new(
                    "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
                    2,
                )
                .unwrap(),
            ],
        );
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(
            s,
            r#"  +---------------------------------------------------+---------------------+
  | Address                                           | XTZ Balance (mutez) |
  +===================================================+=====================+
  | (bootstrap4) tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv | 2                   |
  +---------------------------------------------------+---------------------+
  | tz1hGHtks3PnX4SnpqcDNMg5P3AQhTiH1WE4              | 1                   |
  +---------------------------------------------------+---------------------+

"#
        );
    }

    #[test]
    fn serialize_config() {
        let config = JstzdConfig::new(
            OctezNodeConfigBuilder::new().build().unwrap(),
            OctezBakerConfigBuilder::new()
                .set_binary_path(BakerBinaryPath::Custom(
                    PathBuf::from_str("bin").unwrap(),
                ))
                .set_octez_client_base_dir("base_dir")
                .set_octez_node_endpoint(&Endpoint::default())
                .build()
                .unwrap(),
            OctezClientConfigBuilder::new(Endpoint::default())
                .build()
                .unwrap(),
            OctezRollupConfigBuilder::new(
                Endpoint::default(),
                PathBuf::from("/foo"),
                SmartRollupHash::from_str("sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK")
                    .unwrap(),
                "foo".to_owned(),
                PathBuf::from("/foo"),
            )
            .build()
            .unwrap(),
            JstzNodeConfig::new(
                &Endpoint::default(),
                &Endpoint::default(),
                &PathBuf::from("/foo"),
                &PathBuf::from("/foo"),
                KeyPair::default(),
                jstz_node::RunMode::Default,
                0,
            ),
            ProtocolParameterBuilder::new()
                .set_bootstrap_accounts([BootstrapAccount::new(
                    "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv",
                    6_000_000_000,
                )
                .unwrap()])
                .build()
                .unwrap(),
        );
        let value = serde_json::to_value(config).unwrap();
        let mut keys = value.as_object().unwrap().keys().collect::<Vec<&String>>();
        keys.sort();
        assert_eq!(
            keys,
            [
                "jstz_node",
                "octez_baker",
                "octez_client",
                "octez_node",
                "octez_rollup",
            ]
        );
    }

    #[tokio::test]
    async fn handle_show_address_response_ok() {
        let res = super::handle_show_address_response(Ok(Address {
            hash: PublicKeyHash::from_str("tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv")
                .unwrap(),
            public_key: PublicKey::from_base58(
                "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv",
            )
            .unwrap(),
            secret_key: None,
        }))
        .into_response();
        assert_eq!(res.status(), 200);
        let address =
            String::from_utf8(to_bytes(res.into_body(), 100).await.unwrap().to_vec())
                .unwrap();
        assert_eq!(address, "tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv");
    }

    #[tokio::test]
    async fn handle_show_address_response_unknown_alias() {
        let res = super::handle_show_address_response(Err(anyhow::anyhow!(
            r#"Warning:

                 This is NOT the Tezos Mainnet.

           Do NOT use your fundraiser keys on this network.

Error:
  Erroneous command line argument 3 (test).
  no public key hash alias named test"#
        )))
        .into_response();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn handle_show_address_response_other_errors() {
        let res = super::handle_show_address_response(Err(anyhow::anyhow!(
            r#"Warning:

                 This is NOT the Tezos Mainnet.

           Do NOT use your fundraiser keys on this network.

Error:
  Erroneous command line argument 3 (test).
  Unknown command 'foo'"#
        )))
        .into_response();
        assert_eq!(res.status(), 500);
    }
}
