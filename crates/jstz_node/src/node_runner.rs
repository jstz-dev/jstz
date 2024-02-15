use std::{fs::File, io, path::PathBuf};

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use env_logger::{Builder, Env};
use octez::OctezRollupClient;
use tokio_util::sync::CancellationToken;

use crate::services::{AccountsService, LogsService, OperationsService, Service};

/// Endpoint defailts for the `jstz-node`
const ENDPOINT: (&str, u16) = ("127.0.0.1", 8933);

/*
pub async fn run_node(
    rollup_node_rpc_addr: String,
    rollup_node_rpc_port: u16,
    rollup_endpoint: Option<String>,
    kernel_file_path: String,
    log_file: Option<PathBuf>,
) -> io::Result<()> {
    if let Some(log_file) = log_file {
        let target = Box::new(File::create(log_file).expect("Can't create file"));
        Builder::from_env(Env::default().default_filter_or("info"))
            .target(env_logger::Target::Pipe(target))
            .init();
    }

    let rollup_endpoint = rollup_endpoint.unwrap_or(format!(
        "http://{}:{}",
        rollup_node_rpc_addr, rollup_node_rpc_port
    ));

    let rollup_client = Data::new(OctezRollupClient::new(rollup_endpoint));

    let cancellation_token = CancellationToken::new();

    let (broadcaster, tail_file_handle) =
        LogsService::init(kernel_file_path, &cancellation_token);

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .configure(OperationsService::configure)
            .configure(AccountsService::configure)
            .app_data(Data::from(broadcaster.clone()))
            .configure(LogsService::configure)
            .wrap(Logger::default())
    })
    .bind(ENDPOINT)?
    .run()
    .await?;

    cancellation_token.cancel();

    tail_file_handle.await.unwrap()?;

    Ok(())
}
*/
