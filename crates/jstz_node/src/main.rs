use std::path::PathBuf;

use clap::Parser;
use env_logger::Env;

const DEFAULT_ROLLUP_NODE_RPC_ADDR: &str = "127.0.0.1";
const DEFAULT_ROLLUP_RPC_PORT: u16 = 8932;
const DEFAULT_KERNEL_LOG_PATH: &str = "logs/kernel.log";

// Endpoint defaults for the `jstz-node`
const DEFAULT_JSTZ_NODE_ADDR: &str = "127.0.0.1";
const DEFAULT_JSTZ_NODE_PORT: u16 = 8933;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = DEFAULT_JSTZ_NODE_ADDR)]
    addr: String,

    #[arg(long, default_value_t = DEFAULT_JSTZ_NODE_PORT)]
    port: u16,

    #[arg(long, default_value = DEFAULT_ROLLUP_NODE_RPC_ADDR)]
    rollup_node_rpc_addr: String,

    #[arg(long, default_value_t = DEFAULT_ROLLUP_RPC_PORT)]
    rollup_node_rpc_port: u16,

    #[arg(short, long)]
    rollup_endpoint: Option<String>,

    #[arg(long, default_value = DEFAULT_KERNEL_LOG_PATH)]
    kernel_log_path: PathBuf,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let args = Args::parse();

    let rollup_endpoint = args.rollup_endpoint.unwrap_or(format!(
        "http://{}:{}",
        args.rollup_node_rpc_addr, args.rollup_node_rpc_port
    ));

    jstz_node::run(&args.addr, args.port, rollup_endpoint, args.kernel_log_path).await
}
