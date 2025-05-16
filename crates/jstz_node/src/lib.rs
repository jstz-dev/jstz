use anyhow::Result;
use api_doc::{modify, ApiDoc};
use axum::{
    extract::DefaultBodyLimit,
    http::{self},
    routing::get,
};
use config::{JstzNodeConfig, KeyPair};
use jstz_core::reveal_data::MAX_REVEAL_SIZE;
use octez::OctezRollupClient;
use serde::{Deserialize, Serialize};
use services::{
    accounts::AccountsService,
    logs::{broadcaster::Broadcaster, db::Db, LogsService},
    operations::OperationsService,
    utils,
};
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

mod api_doc;
mod services;
mod tailed_file;
use services::Service;
use tokio_util::sync::CancellationToken;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
pub mod config;
mod sequencer;

#[derive(Clone)]
pub struct AppState {
    pub rollup_client: OctezRollupClient,
    pub rollup_preimages_dir: PathBuf,
    pub broadcaster: Arc<Broadcaster>,
    pub db: Db,
    pub injector: KeyPair,
    pub mode: RunMode,
}

#[derive(Debug, Deserialize, Serialize, Clone, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Sequencer,
    Default,
}

pub async fn run_with_config(config: JstzNodeConfig) -> Result<()> {
    let endpoint_addr = config.endpoint.host();
    let endpoint_port = config.endpoint.port();
    let rollup_endpoint = config.rollup_endpoint.to_string();
    run(
        endpoint_addr,
        endpoint_port,
        rollup_endpoint,
        config.rollup_preimages_dir.to_path_buf(),
        config.kernel_log_file.to_path_buf(),
        config.injector,
        config.mode,
    )
    .await
}

pub async fn run(
    addr: &str,
    port: u16,
    rollup_endpoint: String,
    rollup_preimages_dir: PathBuf,
    kernel_log_path: PathBuf,
    injector: KeyPair,
    mode: RunMode,
) -> Result<()> {
    let rollup_client = OctezRollupClient::new(rollup_endpoint.to_string());

    let cancellation_token = CancellationToken::new();
    let (broadcaster, db, tail_file_handle) =
        LogsService::init(&kernel_log_path, &cancellation_token).await?;

    let state = AppState {
        rollup_client,
        rollup_preimages_dir,
        broadcaster,
        db,
        injector,
        mode,
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
        .layer(DefaultBodyLimit::max(MAX_REVEAL_SIZE))
}

pub fn openapi_json_raw() -> anyhow::Result<String> {
    let mut doc = router().split_for_parts().1;
    modify(&mut doc);
    Ok(doc.to_pretty_json()?)
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use octez::unused_port;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    use crate::{run, KeyPair, RunMode};

    #[test]
    fn api_doc_regression() {
        let _ = include_str!("../openapi.json");
        let filename = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
        let current_spec = std::fs::read_to_string(filename).unwrap();
        let current_spec = current_spec.trim();
        let generated_spec = crate::openapi_json_raw().unwrap();
        assert_eq!(
        current_spec,
        generated_spec,
        "API doc regression detected. Run the 'spec' command to update:\n\tcargo run --bin jstz-node -- spec -o crates/jstz_node/openapi.json"
    );
    }

    #[tokio::test]
    async fn test_run() {
        async fn check_mode(mode: RunMode, expected: &str) {
            let port = unused_port();
            let log_file = NamedTempFile::new().unwrap();
            let h = tokio::spawn(run(
                "0.0.0.0",
                port,
                "0.0.0.0:5678".to_string(),
                PathBuf::new(),
                log_file.path().to_path_buf(),
                KeyPair::default(),
                mode.clone(),
            ));

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
}
