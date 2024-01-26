use std::io::{self, ErrorKind::Other};

use crate::services::{AccountsService, OperationsService, Service};
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use clap::Parser;
use env_logger::Env;
use jstz_cli::sandbox::{
    DEFAULT_ROLLUP_NODE_RPC_ADDR, DEFAULT_ROLLUP_RPC_PORT, ENDPOINT,
};
use octez::OctezRollupClient;
use services::{LogsService, Service};
use tokio_util::sync::CancellationToken;

use crate::{
    node_runner::run_node,
    services::{AccountsService, OperationsService},
};
pub use error::{Error, Result};
use octez::OctezRollupClient;
use services::LogsService;
use tokio_util::sync::CancellationToken;

mod error;
mod node_runner;
mod services;
mod tailed_file;

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
    run_node(
        args.rollup_node_rpc_addr,
        args.rollup_node_rpc_port,
        args.rollup_endpoint,
        args.kernel_file_path,
    )
    .await
}
