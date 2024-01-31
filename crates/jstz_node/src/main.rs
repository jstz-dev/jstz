use std::io::{self, ErrorKind::Other};

use crate::services::{AccountsService, OperationsService, Service};
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use clap::Parser;
use env_logger::Env;
pub use error::{Error, Result};
use octez::OctezRollupClient;
use services::LogsService;
use tokio_util::sync::CancellationToken;

mod error;
mod services;
mod tailed_file;

/// Endpoint defaults for the `octez-smart-rollup-node`
const DEFAULT_ROLLUP_NODE_RPC_ADDR: &str = "127.0.0.1";
const DEFAULT_ROLLUP_RPC_PORT: u16 = 8932;

/// Endpoint defaults for the `jstz-node`
const DEFAULT_JSTZ_NODE_ADDR: &str = "127.0.0.1";
const DEFAULT_JSTZ_NODE_PORT: u16 = 8933;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = DEFAULT_ROLLUP_NODE_RPC_ADDR)]
    rollup_node_rpc_addr: String,

    #[arg(long, default_value_t = DEFAULT_ROLLUP_RPC_PORT)]
    rollup_node_rpc_port: u16,

    #[arg(short, long)]
    rollup_endpoint: Option<String>,

    #[arg(long, default_value = DEFAULT_JSTZ_NODE_ADDR)]
    addr: String,

    #[arg(long, default_value_t = DEFAULT_JSTZ_NODE_PORT)]
    port: u16,

    #[arg(long)]
    kernel_file_path: String,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let rollup_endpoint = args.rollup_endpoint.unwrap_or(format!(
        "http://{}:{}",
        args.rollup_node_rpc_addr, args.rollup_node_rpc_port
    ));

    let rollup_client = Data::new(OctezRollupClient::new(rollup_endpoint));

    let cancellation_token = CancellationToken::new();

    let (broadcaster, db, tail_file_handle) =
        LogsService::init(args.kernel_file_path, &cancellation_token)
            .await
            .map_err(|e| io::Error::new(Other, e.to_string()))?;

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .configure(OperationsService::configure)
            .configure(AccountsService::configure)
            .app_data(Data::from(broadcaster.clone()))
            .app_data(Data::new(db.clone()))
            .configure(LogsService::configure)
            .wrap(Logger::default())
    })
    .bind((args.addr, args.port))?
    .run()
    .await?;

    cancellation_token.cancel();

    tail_file_handle.await.unwrap()?;

    Ok(())
}
