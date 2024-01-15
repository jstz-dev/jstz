use std::io;

use crate::services::{AccountsService, OperationsService};
use actix_web::{middleware::Logger, web::Data, App, HttpServer, Scope};
use clap::Parser;
use env_logger::Env;
pub use error::{Error, Result};
use octez::OctezRollupClient;
use services::{logs::stream_logs, LogsService};
use tokio_util::sync::CancellationToken;

mod error;
mod services;
mod tailed_file;

/// Endpoint details for the `octez-smart-rollup-node`
const DEFAULT_ROLLUP_NODE_RPC_ADDR: &str = "127.0.0.1";
const DEFAULT_ROLLUP_RPC_PORT: u16 = 8932;

/// Endpoint defailts for the `jstz-node`
const ENDPOINT: (&str, u16) = ("127.0.0.1", 8933);

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = DEFAULT_ROLLUP_NODE_RPC_ADDR)]
    rollup_node_rpc_addr: String,

    #[arg(long, default_value_t = DEFAULT_ROLLUP_RPC_PORT)]
    rollup_node_rpc_port: u16,

    #[arg(short, long)]
    rollup_endpoint: Option<String>,

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

    // todo: add db_file_path to args;
    let db_path = dirs::home_dir()
        .expect("Failed to get home directory")
        .join(".jstz")
        .join("log.db")
        .to_str()
        .unwrap()
        .to_owned();

    let (broadcaster, tail_file_handle) =
        LogsService::init(args.kernel_file_path, db_path, &cancellation_token);

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .service(
                Scope::new("/logs")
                    .app_data(Data::from(broadcaster.clone()))
                    .service(stream_logs),
            )
            .wrap(Logger::default())
            .configure(OperationsService::configure)
            .configure(AccountsService::configure)
    })
    .bind(ENDPOINT)?
    .run()
    .await?;

    cancellation_token.cancel();

    tail_file_handle.await.unwrap()?;

    Ok(())
}
