use anyhow::{Context, Result};
use api_doc::{modify, ApiDoc};
use axum::{extract::DefaultBodyLimit, http, routing::get};
use config::JstzNodeConfig;
use jstz_core::reveal_data::MAX_REVEAL_SIZE;
use jstz_utils::KeyPair;
use octez::OctezRollupClient;
#[cfg(not(test))]
use sequencer::inbox;
use sequencer::{inbox::Monitor, queue::OperationQueue, worker};
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
use tokio::{net::TcpListener, task::JoinSet};
use tower_http::cors::{Any, CorsLayer};

mod api_doc;
mod services;
pub mod storage_sync;
use services::Service;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
pub mod config;
pub mod sequencer;
pub use config::RunMode;

use crate::config::RuntimeEnv;

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
    #[cfg(feature = "blueprint")]
    pub blueprint_db: sequencer::db::BlueprintDb,
    worker_heartbeat: Arc<AtomicU64>,
    storage_sync: bool,
    storage_sync_db: sequencer::db::Db,
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

pub struct RunOptions {
    pub addr: String,
    pub port: u16,
    pub rollup_endpoint: String,
    pub rollup_preimages_dir: PathBuf,
    pub kernel_log_path: PathBuf,
    pub injector: KeyPair,
    pub mode: RunMode,
    pub storage_sync: bool,
    pub runtime_db_path: Option<PathBuf>,
    #[cfg(feature = "blueprint")]
    pub blueprint_db_path: PathBuf,
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
        storage_sync: config.storage_sync,
        runtime_db_path: config.runtime_db_path,
        #[cfg(feature = "blueprint")]
        blueprint_db_path: config.blueprint_db_file,
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
        storage_sync,
        runtime_db_path,
        #[cfg(feature = "blueprint")]
        blueprint_db_path,
    }: RunOptions,
) -> Result<()> {
    let rollup_client = OctezRollupClient::new(rollup_endpoint.to_string());
    let queue = Arc::new(RwLock::new(OperationQueue::new(match mode {
        RunMode::Sequencer { capacity, .. } => capacity,
        _ => 0,
    })));

    // When runtime_db_path is not provided, the db is created with a temp file rather than
    // with the in-memory setup to keep the behaviour consistent and avoid consuming
    // too much memory unexpectedly. If somehow path-to-str conversion fails, the in-memory
    // setup will be used as the fallback option.
    // `_tmp_file` simply holds the temporary file so that it gets cleaned up when the node
    // is shut down.
    let (db_path, _tmp_file) = match runtime_db_path {
        Some(p) => (p, None),
        None => {
            let f = NamedTempFile::new()?;
            (f.path().to_path_buf(), Some(f))
        }
    };
    let runtime_db = sequencer::db::Db::init(db_path.as_path().to_str())?;

    #[cfg(feature = "blueprint")]
    let blueprint_db =
        sequencer::db::BlueprintDb::init(Some(blueprint_db_path.to_str().ok_or(
            anyhow::anyhow!("failed to convert temp db file path to str"),
        )?))?;

    let worker = match mode {
        #[cfg(not(test))]
        RunMode::Sequencer {
            ref debug_log_path,
            ref runtime_env,
            ref rollup_address,
            ..
        } => Some(
            worker::spawn(
                queue.clone(),
                runtime_db.clone(),
                rollup_address,
                &injector,
                rollup_preimages_dir.clone(),
                Some(debug_log_path),
                runtime_env,
                #[cfg(feature = "blueprint")]
                blueprint_db.clone(),
            )
            .context("failed to launch worker")?,
        ),
        #[cfg(test)]
        RunMode::Sequencer {
            ref debug_log_path,
            ref runtime_env,
            ref rollup_address,
            ..
        } => {
            let p = rollup_preimages_dir.join(format!("{rollup_endpoint}.txt"));
            Some(
                worker::spawn(
                    queue.clone(),
                    runtime_db.clone(),
                    rollup_address,
                    &injector,
                    rollup_preimages_dir.clone(),
                    Some(debug_log_path),
                    runtime_env,
                    #[cfg(feature = "blueprint")]
                    blueprint_db.clone(),
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
        RunMode::Sequencer {
            ref inbox_checkpoint_path,
            ref ticketer_address,
            ref rollup_address,
            ..
        } => Some(
            inbox::spawn_monitor(
                rollup_endpoint,
                rollup_address.clone(),
                ticketer_address.clone(),
                queue.clone(),
                inbox_checkpoint_path.clone(),
            )
            .await?,
        ),
        #[cfg(test)]
        RunMode::Sequencer { .. } => None,
        RunMode::Default => None,
    };

    // LogsService expects the log file to exist at instantiation, so this needs to be called after
    // debug log file is created.
    let log_file_path = match mode {
        RunMode::Default => kernel_log_path.clone(),
        RunMode::Sequencer {
            ref debug_log_path, ..
        } => debug_log_path.clone(),
    };

    let (broadcaster, db, log_service_handle) = LogsService::init(&log_file_path).await?;

    let (storage_sync_db, _storage_sync_db_file) = temp_db()?;
    let mut storage_sync_handles = JoinSet::new();
    if storage_sync {
        storage_sync_handles.spawn(storage_sync::spawn(
            storage_sync_db.clone(),
            kernel_log_path.clone(),
            #[cfg(test)]
            || {},
        )?);
    };

    if let RunMode::Sequencer {
        debug_log_path,
        runtime_env: RuntimeEnv::Riscv { .. },
        ..
    } = &mode
    {
        storage_sync_handles.spawn(storage_sync::spawn(
            runtime_db.clone(),
            debug_log_path.to_owned(),
            #[cfg(test)]
            || {},
        )?);
    };

    let state = AppState {
        rollup_client,
        rollup_preimages_dir,
        broadcaster,
        db,
        injector,
        mode,
        queue,
        runtime_db,
        #[cfg(feature = "blueprint")]
        blueprint_db,
        worker_heartbeat: worker.as_ref().map(|w| w.heartbeat()).unwrap_or_default(),
        storage_sync,
        storage_sync_db,
    };

    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let (router, mut openapi) = router().with_state(state).layer(cors).split_for_parts();
    modify(&mut openapi);
    let router = router.merge(Scalar::with_url("/scalar", openapi));

    let listener = TcpListener::bind(format!("{addr}:{port}")).await?;

    match storage_sync_handles.is_empty() {
        false => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let sig = match storage_sync_handles.join_next().await {
                        Some(Ok(v)) => v,
                        Some(Err(e)) => Err(e.into()),
                        None => Ok(()), // should not reach here actually as storage_sync_handles is confirmed non-empty
                    };
                    let _ = tx.send(sig);
                    // kill other storage sync instances
                    drop(storage_sync_handles);
                })
                .await?;
            rx.await??;
        }
        true => axum::serve(listener, router).await?,
    };

    log_service_handle.shutdown().await?;
    Ok(())
}

fn temp_db() -> Result<(sequencer::db::Db, NamedTempFile)> {
    let db_file = NamedTempFile::new()?;
    let db_path = db_file.path().to_str().ok_or(anyhow::anyhow!(
        "failed to convert temp db file path to str"
    ))?;
    Ok((sequencer::db::Db::init(Some(db_path))?, db_file))
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

    use jstz_core::{event::StringEncodable, kv::storage_update::BatchStorageUpdate};
    use jstz_crypto::{
        hash::Hash, public_key::PublicKey, public_key_hash::PublicKeyHash,
        secret_key::SecretKey,
    };
    use jstz_proto::context::account::{Account, Nonce, UserAccount};
    use jstz_utils::test_util::append_async;
    use octez::unused_port;
    use pretty_assertions::assert_eq;
    use tempfile::{NamedTempFile, TempDir};
    use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
    use tezos_smart_rollup::storage::path::OwnedPath;
    use tokio::{
        task::yield_now,
        time::{sleep, timeout, Duration},
    };

    use crate::{
        config::RuntimeEnv,
        run,
        services::utils::tests::mock_app_state,
        storage_sync::tests::{make_line, KILL_KEY},
        KeyPair, RunMode, RunOptions,
    };

    pub fn default_injector() -> KeyPair {
        KeyPair(
            PublicKey::from_base58(
                "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            )
            .unwrap(),
            SecretKey::from_base58(
                "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            )
            .unwrap(),
        )
    }

    #[test]
    fn api_doc_regression() {
        let _ = include_str!("../openapi.json");
        #[cfg(feature = "v2_runtime")]
        let filename = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
        #[cfg(not(feature = "v2_runtime"))]
        let filename = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi_v1.json");
        let current_spec = std::fs::read_to_string(filename).unwrap();
        let current_spec = current_spec.trim();
        let generated_spec = crate::openapi_json_raw().unwrap();
        #[cfg(feature = "v2_runtime")]
        assert!(
            current_spec == generated_spec,
            "API doc regression detected. Run the following to view the modifications:\n\tcargo run --bin jstz-node --features v2_runtime -- spec -o crates/jstz_node/openapi.json"
        );
        #[cfg(not(feature = "v2_runtime"))]
        assert!(
            current_spec == generated_spec,
            "API doc regression detected. Run the following to view the modifications:\n\tcargo run --bin jstz-node -- spec -o crates/jstz_node/openapi_v1.json"
        );
    }

    #[tokio::test]
    async fn test_run() {
        async fn check_mode(mode: RunMode, expected: &str) {
            let port = unused_port();
            let kernel_log_file = NamedTempFile::new().unwrap();

            let h = tokio::spawn(run(RunOptions {
                addr: "0.0.0.0".to_string(),
                port,
                rollup_endpoint: "0.0.0.0:5678".to_string(),
                rollup_preimages_dir: TempDir::new().unwrap().into_path(),
                kernel_log_path: kernel_log_file.path().to_path_buf(),
                injector: default_injector(),
                mode: mode.clone(),
                storage_sync: false,
                runtime_db_path: None,
                #[cfg(feature = "blueprint")]
                blueprint_db_path: NamedTempFile::new().unwrap().path().to_path_buf(),
            }));

            let res = jstz_utils::poll(10, 500, || async {
                reqwest::get(format!("http://0.0.0.0:{port}/mode"))
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

        // Test without oracle key pair
        check_mode(RunMode::Default, "\"default\"").await;
        check_mode(
            RunMode::Sequencer {
                capacity: 0,
                debug_log_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                runtime_env: RuntimeEnv::Native,
                inbox_checkpoint_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                ticketer_address: ContractKt1Hash::from_base58_check(
                    "KT1ChNsEFxwyCbJyWGSL3KdjeXE28AY1Kaog",
                )
                .unwrap(),
                rollup_address: SmartRollupHash::from_base58_check(
                    "sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao",
                )
                .unwrap(),
            },
            "\"sequencer\"",
        )
        .await;
    }

    #[tokio::test]
    async fn worker() {
        async fn run_test(
            rollup_preimages_dir: PathBuf,
            rollup_endpoint: String,
            mode: RunMode,
            #[allow(unused_variables)] with_oracle: bool,
        ) {
            let port = unused_port();
            let kernel_log_file = NamedTempFile::new().unwrap();

            let h = tokio::spawn(run(RunOptions {
                addr: "0.0.0.0".to_string(),
                port,
                rollup_endpoint,
                rollup_preimages_dir,
                kernel_log_path: kernel_log_file.path().to_path_buf(),
                injector: default_injector(),
                mode,
                storage_sync: false,
                runtime_db_path: None,
                #[cfg(feature = "blueprint")]
                blueprint_db_path: NamedTempFile::new().unwrap().path().to_path_buf(),
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
            RunMode::Sequencer {
                capacity: 0,
                debug_log_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                runtime_env: RuntimeEnv::Native,
                inbox_checkpoint_path: NamedTempFile::new().unwrap().path().to_path_buf(),
                ticketer_address: ContractKt1Hash::from_base58_check(
                    "KT1ChNsEFxwyCbJyWGSL3KdjeXE28AY1Kaog",
                )
                .unwrap(),
                rollup_address: SmartRollupHash::from_base58_check(
                    "sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao",
                )
                .unwrap(),
            },
            false,
        )
        .await;
        // the test worker's on_exit function should be called on drop and
        // it should create this file
        assert!(preimages_dir.join("sequencer-test-file.txt").exists());

        // Test default mode without oracle
        run_test(
            preimages_dir.clone(),
            "default-test-file".to_string(),
            RunMode::Default,
            false,
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

    // Make a storage update that sets the balance of the `addr` to `amount`
    fn sets_balance(addr: PublicKeyHash, amount: u64) -> BatchStorageUpdate {
        let mut updates = BatchStorageUpdate::new(1);
        let key =
            OwnedPath::try_from(format!("/jstz_account/{}", addr.to_base58())).unwrap();
        let val = Account::User(UserAccount {
            amount,
            nonce: Nonce(0),
        });
        updates.push_insert(&key, &val).unwrap();
        updates
    }

    // Make a storage update that kills the storage sync for testing
    fn kills_storage_sync() -> BatchStorageUpdate {
        let mut updates = BatchStorageUpdate::new(1);
        let key = OwnedPath::try_from(KILL_KEY.to_string()).unwrap();
        updates.push_remove(&key);
        updates
    }

    fn start_server_with_storage_sync(
        port: u16,
        kernel_log_file: &NamedTempFile,
        mode: RunMode,
    ) -> tokio::task::JoinHandle<Result<(), anyhow::Error>> {
        tokio::spawn(run(RunOptions {
            addr: "0.0.0.0".to_string(),
            port,
            rollup_endpoint: "".to_string(),
            rollup_preimages_dir: TempDir::new().unwrap().into_path(),
            kernel_log_path: kernel_log_file.path().to_path_buf(),
            injector: default_injector(),
            mode,
            storage_sync: true,
            runtime_db_path: None,
            #[cfg(feature = "blueprint")]
            blueprint_db_path: NamedTempFile::new().unwrap().path().to_path_buf(),
        }))
    }

    #[tokio::test]
    async fn storage_sync_spawn() -> anyhow::Result<()> {
        let port = unused_port();
        let kernel_log_file = NamedTempFile::new().unwrap();
        let addr =
            PublicKeyHash::from_base58("tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9").unwrap();
        let amount = 10_000_000;

        let _server: tokio::task::JoinHandle<Result<(), anyhow::Error>> =
            start_server_with_storage_sync(port, &kernel_log_file, RunMode::Default);

        // wait for the server to start
        sleep(Duration::from_secs(3)).await;

        let storage_updates = sets_balance(addr.clone(), amount);
        // write a storage update that deposits the `amount` to the `addr`
        let writer = tokio::spawn(append_async(
            kernel_log_file.path().to_path_buf(),
            make_line(&storage_updates),
            25,
        ));

        // Check that the storage update is applied to the database
        timeout(Duration::from_secs(3), async {
            loop {
                let b = reqwest::get(format!(
                    "http://0.0.0.0:{port}/accounts/{addr}/balance"
                ))
                .await
                .ok();
                if let Some(b) = b {
                    if b.status().is_success() {
                        let balance = b.text().await.unwrap();
                        assert_eq!(u64::from_str(&balance).unwrap(), amount);
                        break;
                    }
                }
                yield_now().await;
            }
        })
        .await
        .expect("should update balance");
        writer.await??;
        Ok(())
    }

    #[tokio::test]
    async fn server_dies_when_storage_sync_dies() -> anyhow::Result<()> {
        let port = unused_port();
        let kernel_log_file = NamedTempFile::new().unwrap();
        PublicKeyHash::from_base58("tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9").unwrap();

        let server: tokio::task::JoinHandle<Result<(), anyhow::Error>> =
            start_server_with_storage_sync(port, &kernel_log_file, RunMode::Default);

        // wait for the server to start
        sleep(Duration::from_secs(2)).await;

        // check health
        let health_endpoint = format!("http://0.0.0.0:{port}/health");
        let res = reqwest::get(health_endpoint.clone()).await?;
        let status = res.status();
        assert!(status.is_success());

        // write a storage update that kills the storage sync
        let writer = tokio::spawn(append_async(
            kernel_log_file.path().to_path_buf(),
            make_line(&kills_storage_sync()),
            25,
        ));

        // Wait for the server to shutdown.
        sleep(Duration::from_secs(2)).await;

        // server dies after storage sync dies - should get a connection error
        timeout(Duration::from_secs(2), async {
            loop {
                if let Err(e) = &reqwest::get(health_endpoint.clone()).await {
                    assert!(e.is_connect(), "Expected connection error, got: {e:?}");
                    break;
                }
                yield_now().await;
            }
        })
        .await?;
        writer.await??;

        // server propagates the error from storage sync
        assert!(server
            .await?
            .is_err_and(|e| e.to_string().contains("received test kill signal")));
        Ok(())
    }
}
