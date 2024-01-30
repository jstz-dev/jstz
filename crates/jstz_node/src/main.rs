use std::io::{self, ErrorKind::Other};

<<<<<<< HEAD
use crate::services::{AccountsService, OperationsService, Service};
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
=======
>>>>>>> fd96ac4 (feat(node): running node in sandbox)
use clap::Parser;

use crate::{
    config::{
        DEFAULT_KERNEL_FILE_PATH, DEFAULT_ROLLUP_NODE_RPC_ADDR, DEFAULT_ROLLUP_RPC_PORT,
    },
    node_runner::run_node,
};
pub use error::{Error, Result};
use octez::OctezRollupClient;
use services::LogsService;
use tokio_util::sync::CancellationToken;

mod config;
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

    #[arg(long, default_value = DEFAULT_KERNEL_FILE_PATH)]
    kernel_file_path: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    run_node(
        args.rollup_node_rpc_addr,
        args.rollup_node_rpc_port,
        args.rollup_endpoint,
        args.kernel_file_path,
        true,
    )
    .await
}
