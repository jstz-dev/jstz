use std::io;

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use clap::Parser;
use env_logger::Env;

use crate::{rollup::RollupClient, services::OperationsSerivce};
pub use error::{Error, Result};

mod error;
mod rollup;
mod services;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    rollup_node_rpc_addr: String,

    #[arg(long, default_value_t = 8932)]
    rollup_node_rpc_port: u16,

    #[arg(short, long)]
    rollup_endpoint: Option<String>,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let rollup_endpoint = args.rollup_endpoint.unwrap_or(format!(
        "http://{}:{}",
        args.rollup_node_rpc_addr, args.rollup_node_rpc_port
    ));

    let rollup_client = Data::new(RollupClient::new(rollup_endpoint));

    HttpServer::new(move || {
        App::new()
            .app_data(rollup_client.clone())
            .wrap(Logger::default())
            .configure(OperationsSerivce::configure)
    })
    .bind(("127.0.0.1", 8933))?
    .run()
    .await
}
