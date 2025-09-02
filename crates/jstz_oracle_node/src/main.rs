use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;
#[cfg(feature = "v2_runtime")]
use jstz_oracle_node::node::OracleNode;
use jstz_utils::key_pair::{parse_key_file, KeyPair};

const DEFAULT_JSTZ_NODE_ENDPOINT: &str = "http://127.0.0.1:8933";

#[derive(Debug, Parser)]
#[command(name = "jstz-oracle-node")]
#[command(about = "JSTZ Oracle Node - Provides oracle data for JSTZ rollup")]
struct Args {
    /// Path to the log file
    #[arg(long)]
    log_path: PathBuf,

    /// JSTZ node endpoint
    #[arg(long, default_value = DEFAULT_JSTZ_NODE_ENDPOINT)]
    node_endpoint: String,

    /// Path to file containing key pair (format: {"public_key": ..., "secret_key": ...})
    #[arg(long)]
    key_file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let args = Args::parse();

    // Check if log path exists
    if !args.log_path.exists() {
        anyhow::bail!("Log path does not exist: {:?}", args.log_path);
    }

    // Canonicalize log path
    let canonical_log_path = args
        .log_path
        .canonicalize()
        .context("Failed to canonicalize log path")?;

    // Parse key file
    let KeyPair(public_key, _secret_key) =
        parse_key_file(args.key_file).context("failed to parse key file")?;

    log::info!("Starting JSTZ Oracle Node");
    log::info!(
        "Listening for Oracle request events on: {:?}",
        canonical_log_path
    );
    log::info!("Node endpoint: {}", args.node_endpoint);
    log::info!("Public key: {}", public_key.to_base58());

    // Spawn the oracle node
    #[cfg(feature = "v2_runtime")]
    {
        let _oracle_node = OracleNode::spawn(
            canonical_log_path,
            public_key,
            _secret_key,
            args.node_endpoint,
        )
        .await
        .context("Failed to spawn oracle node")?;

        log::info!("Oracle node started successfully");

        // Keep the node running. The node will keep running until this task is dropped
        // If the Relay or DataProvider dies, the main thread will not be notified. With the
        // current implementation, it is not possible for them to die. This might change in
        // the future.
        tokio::signal::ctrl_c()
            .await
            .context("Failed to wait for Ctrl+C")?;

        log::info!("Shutting down oracle node...");

        Ok(())
    }
    #[cfg(not(feature = "v2_runtime"))]
    {
        anyhow::bail!("Oracle node is not supported in this runtime");
    }
}
