use std::io;

use actix_web::{middleware::Logger, web::Data, App, HttpServer};

use octez::OctezRollupClient;
use services::{LogsService, Service};
use tokio_util::sync::CancellationToken;
use utoipa::OpenApi;

use crate::services::{AccountsService, OperationsService};
pub use error::{Error, Result};

mod error;
mod services;
mod tailed_file;

pub fn docs() -> utoipa::openapi::OpenApi {
    #[derive(OpenApi)]
    #[openapi(
        paths(services::operations::inject,),
        components(schemas(jstz_proto::operation::SignedOperation),)
    )]
    struct ApiDoc;

    ApiDoc::openapi()
}

pub async fn run(
    addr: &str,
    port: u16,
    rollup_endpoint: &str,
    kernel_log_path: &str,
) -> io::Result<()> {
    let rollup_client = Data::new(OctezRollupClient::new(rollup_endpoint.to_string()));

    let cancellation_token = CancellationToken::new();

    let (broadcaster, tail_file_handle) =
        LogsService::init(kernel_log_path, &cancellation_token);

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .configure(OperationsService::configure)
            .configure(AccountsService::configure)
            .app_data(Data::from(broadcaster.clone()))
            .configure(LogsService::configure)
            .wrap(Logger::default())
    })
    .bind((addr, port))?
    .run()
    .await?;

    cancellation_token.cancel();
    tail_file_handle.await.expect("Unexpected error joining")?;

    Ok(())
}
