use anyhow::{Context, Result};
use api_doc::{modify, ApiDoc};
use axum::{extract::DefaultBodyLimit, http, routing::get};
use config::{JstzNodeConfig, KeyPair};
use jstz_core::reveal_data::MAX_REVEAL_SIZE;
use octez::OctezRollupClient;
#[cfg(not(test))]
use sequencer::inbox;
use sequencer::{inbox::Monitor, queue::OperationQueue, worker};
use serde::{Deserialize, Serialize};
use services::{
    accounts::AccountsService,
    logs::{broadcaster::Broadcaster, db::Db, LogsService},
    operations::OperationsService,
    utils,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc, RwLock},
    time::SystemTime,
};
use tempfile::NamedTempFile;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

mod api_doc;
mod services;
use services::Service;
use tokio_util::sync::CancellationToken;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
pub mod config;
pub mod sequencer;

#[derive(Clone)]
pub struct AppState {
    pub rollup_client: OctezRollupClient,
    pub rollup_preimages_dir: PathBuf,
    pub broadcaster: Arc<Broadcaster>,
    pub db: Db,
    pub injector: KeyPair,
    pub mode: RunMode,
    pub queue: Arc<RwLock<OperationQueue>>,
    pub runtime_db: sequencer::db::Db,
    worker_heartbeat: Arc<AtomicU64>,
}

impl AppState {
    pub fn is_worker_healthy(&self) -> bool {
        let current_sec = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // safety: there is only one writer -- the worker itself.
        let diff = current_sec
            - self
                .worker_heartbeat
                .load(std::sync::atomic::Ordering::Relaxed);
        diff <= 30
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, clap::ValueEnum, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Sequencer,
    #[serde(alias = "default")]
    Default,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Default
    }
}

pub struct RunOptions {
    pub addr: String,
    pub port: u16,
    pub rollup_endpoint: String,
    pub rollup_preimages_dir: PathBuf,
    pub kernel_log_path: PathBuf,
    pub injector: KeyPair,
    pub mode: RunMode,
    pub capacity: usize,
    pub debug_log_path: PathBuf,
}

pub async fn run_with_config(config: JstzNodeConfig) -> Result<()> {
    let endpoint_addr = config.endpoint.host();
    let endpoint_port = config.endpoint.port();
    let rollup_endpoint = config.rollup_endpoint.to_string();
    run(RunOptions {
        addr: endpoint_addr.to_string(),
        port: endpoint_port,
        rollup_endpoint,
        rollup_preimages_dir: config.rollup_preimages_dir.to_path_buf(),
        kernel_log_path: config.kernel_log_file.to_path_buf(),
        injector: config.injector,
        mode: config.mode,
        capacity: config.capacity,
        debug_log_path: config.debug_log_file,
    })
    .await
}

pub async fn run(
    RunOptions {
        addr,
        port,
        rollup_endpoint,
        rollup_preimages_dir,
        kernel_log_path,
        injector,
        mode,
        capacity,
        debug_log_path,
    }: RunOptions,
) -> Result<()> {
    let rollup_client = OctezRollupClient::new(rollup_endpoint.to_string());
    let queue = Arc::new(RwLock::new(OperationQueue::new(capacity)));

    // will make db_path configurable later
    let db_file = NamedTempFile::new()?;
    let db_path = db_file.path().to_str().ok_or(anyhow::anyhow!(
        "failed to convert temp db file path to str"
    ))?;
    let runtime_db = sequencer::db::Db::init(Some(db_path))?;
    let worker = match mode {
        #[cfg(not(test))]
        RunMode::Sequencer => Some(
            worker::spawn(
                queue.clone(),
                runtime_db.clone(),
                &injector,
                rollup_preimages_dir.clone(),
                Some(&debug_log_path),
            )
            .context("failed to launch worker")?,
        ),
        #[cfg(test)]
        RunMode::Sequencer => {
            let p = rollup_preimages_dir.join(format!("{rollup_endpoint}.txt"));
            Some(
                worker::spawn(
                    queue.clone(),
                    runtime_db.clone(),
                    &injector,
                    rollup_preimages_dir.clone(),
                    Some(&debug_log_path),
                    move || {
                        std::fs::File::create(p).unwrap();
                    },
                )
                .context("failed to launch worker")?,
            )
        }
        RunMode::Default => None,
    };

    let _monitor: Option<Monitor> = match mode {
        #[cfg(not(test))]
        RunMode::Sequencer => {
            Some(inbox::spawn_monitor(rollup_endpoint, queue.clone()).await?)
        }
        #[cfg(test)]
        RunMode::Sequencer => None,
        RunMode::Default => None,
    };

    let cancellation_token = CancellationToken::new();
    // LogsService expects the log file to exist at instantiation, so this needs to be called after
    // debug log file is created.
    let (broadcaster, db, tail_file_handle) = LogsService::init(
        match mode {
            RunMode::Default => &kernel_log_path,
            RunMode::Sequencer => &debug_log_path,
        },
        &cancellation_token,
    )
    .await?;

    let state = AppState {
        rollup_client,
        rollup_preimages_dir,
        broadcaster,
        db,
        injector,
        mode,
        queue,
        runtime_db,
        worker_heartbeat: worker.as_ref().map(|w| w.heartbeat()).unwrap_or_default(),
    };

    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let (router, mut openapi) = router().with_state(state).layer(cors).split_for_parts();
    modify(&mut openapi);
    let router = router.merge(Scalar::with_url("/scalar", openapi));

    let listener = TcpListener::bind(format!("{}:{}", addr, port)).await?;
    axum::serve(listener, router).await?;

    cancellation_token.cancel();
    tail_file_handle.await.unwrap()?;
    Ok(())
}

fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(OperationsService::router_with_openapi())
        .merge(AccountsService::router_with_openapi())
        .merge(LogsService::router_with_openapi())
        .route("/mode", get(utils::get_mode))
        .route("/health", get(http::StatusCode::OK))
        .route("/worker/health", get(utils::worker_health))
        .layer(DefaultBodyLimit::max(MAX_REVEAL_SIZE))
}

pub fn openapi_json_raw() -> anyhow::Result<String> {
    let mut doc = router().split_for_parts().1;
    modify(&mut doc);
    Ok(doc.to_pretty_json()?)
}

#[cfg(test)]
mod test {
    use std::{
        path::PathBuf,
        sync::{atomic::AtomicU64, Arc},
        time::SystemTime,
    };

    use octez::unused_port;
    use pretty_assertions::assert_eq;
    use tempfile::{NamedTempFile, TempDir};
    use tokio::time::{sleep, Duration};

    use crate::{
        run, services::utils::tests::mock_app_state, KeyPair, RunMode, RunOptions,
    };

    /*#[test]
    fn api_doc_regression() {
        let _ = include_str!("../openapi.json");
        let filename = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
        let current_spec = std::fs::read_to_string(filename).unwrap();
        let current_spec = current_spec.trim();
        let generated_spec = crate::openapi_json_raw().unwrap();
        assert!(
            current_spec == generated_spec,
            "API doc regression detected. Run the following to view the modifications:\n\tcargo run --bin jstz-node -- spec -o crates/jstz_node/openapi.json"
        );
    }*/

    #[test]
    fn default_runmode() {
        assert_eq!(RunMode::default(), RunMode::Default);
    }

    #[tokio::test]
    async fn test_run() {
        async fn check_mode(mode: RunMode, expected: &str) {
            let port = unused_port();
            let kernel_log_file = NamedTempFile::new().unwrap();
            let debug_log_file = NamedTempFile::new().unwrap();
            let h = tokio::spawn(run(RunOptions {
                addr: "0.0.0.0".to_string(),
                port,
                rollup_endpoint: "0.0.0.0:5678".to_string(),
                rollup_preimages_dir: TempDir::new().unwrap().into_path(),
                kernel_log_path: kernel_log_file.path().to_path_buf(),
                injector: KeyPair::default(),
                mode: mode.clone(),
                capacity: 0,
                debug_log_path: debug_log_file.path().to_path_buf(),
            }));

            let res = jstz_utils::poll(10, 500, || async {
                reqwest::get(format!("http://0.0.0.0:{}/mode", port))
                    .await
                    .ok()
            })
            .await
            .expect("should get response")
            .text()
            .await
            .expect("should get text body");

            assert_eq!(
                res, expected,
                "expecting '{expected}' for mode '{mode:?}' but got '{res}'"
            );

            h.abort();
        }

        check_mode(RunMode::Default, "\"default\"").await;
        check_mode(RunMode::Sequencer, "\"sequencer\"").await;
    }

    #[tokio::test]
    async fn worker() {
        async fn run_test(
            rollup_preimages_dir: PathBuf,
            rollup_endpoint: String,
            mode: RunMode,
        ) {
            let port = unused_port();
            let kernel_log_file = NamedTempFile::new().unwrap();
            let debug_log_file = NamedTempFile::new().unwrap();
            let h = tokio::spawn(run(RunOptions {
                addr: "0.0.0.0".to_string(),
                port,
                rollup_endpoint,
                rollup_preimages_dir,
                kernel_log_path: kernel_log_file.path().to_path_buf(),
                injector: KeyPair::default(),
                mode,
                capacity: 0,
                debug_log_path: debug_log_file.path().to_path_buf(),
            }));

            sleep(Duration::from_secs(1)).await;
            h.abort();
            // wait for the worker in run to be dropped
            sleep(Duration::from_secs(2)).await;
        }
        let preimages_dir = TempDir::new().unwrap().into_path();

        run_test(
            preimages_dir.clone(),
            "sequencer-test-file".to_string(),
            RunMode::Sequencer,
        )
        .await;
        // the test worker's on_exit function should be called on drop and
        // it should create this file
        assert!(preimages_dir.join("sequencer-test-file.txt").exists());

        run_test(
            preimages_dir.clone(),
            "default-test-file".to_string(),
            RunMode::Default,
        )
        .await;
        assert!(!preimages_dir.join("default-test-file.txt").exists());
    }

    #[tokio::test]
    async fn worker_heartbeat() {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut state =
            mock_app_state("", PathBuf::default(), "", RunMode::Default).await;
        state.worker_heartbeat = Arc::new(AtomicU64::new(now - 60));
        // heartbeat is too old
        assert!(!state.is_worker_healthy());

        let mut state =
            mock_app_state("", PathBuf::default(), "", RunMode::Default).await;
        state.worker_heartbeat = Arc::new(AtomicU64::new(now - 5));
        // heartbeat is recent enough
        assert!(state.is_worker_healthy());
    }
}
