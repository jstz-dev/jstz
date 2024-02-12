use std::io::{self, ErrorKind::Other};

use std::path::Path;

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use octez::OctezRollupClient;
use tokio_util::sync::CancellationToken;

mod error;
mod services;
mod tailed_file;

use services::{AccountsService, LogsService, OperationsService, Service};

pub use error::{Error, Result};
pub use services::logs;

pub async fn run(
    addr: &str,
    port: u16,
    rollup_endpoint: &str,
    kernel_log_path: &Path,
) -> anyhow::Result<()> {
    let rollup_client = Data::new(OctezRollupClient::new(rollup_endpoint.to_string()));

    let cancellation_token = CancellationToken::new();

    let (broadcaster, db, tail_file_handle) =
        LogsService::init(kernel_log_path, &cancellation_token)
            .await
            .map_err(|e| io::Error::new(Other, e.to_string()))?;

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .app_data(Data::from(broadcaster.clone()))
            .app_data(Data::new(db.clone()))
            .configure(OperationsService::configure)
            .configure(AccountsService::configure)
            .configure(LogsService::configure)
            .wrap(Logger::default())
    })
    .bind((addr, port))?
    .run()
    .await?;

    cancellation_token.cancel();

    tail_file_handle.await.unwrap()?;

    Ok(())
}
