use anyhow::Result;
use api_doc::{modify, ApiDoc};
use axum::{
<<<<<<< HEAD
    extract::DefaultBodyLimit,
=======
>>>>>>> ac93d01e (feat(jstz_node): prepare rlp operation)
    http::{self},
    routing::get,
};
use config::{JstzNodeConfig, KeyPair};
use jstz_core::reveal_data::MAX_REVEAL_SIZE;
use octez::OctezRollupClient;
use services::{
    accounts::AccountsService,
    logs::{broadcaster::Broadcaster, db::Db, LogsService},
    operations::OperationsService,
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

#[derive(Clone)]
pub struct AppState {
    pub rollup_client: OctezRollupClient,
    pub rollup_preimages_dir: PathBuf,
    pub broadcaster: Arc<Broadcaster>,
    pub db: Db,
    pub injector: KeyPair,
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

    use pretty_assertions::assert_eq;

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
}
