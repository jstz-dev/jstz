mod config;
pub mod docker;
pub mod task;

use crate::task::jstzd::{JstzdConfig, JstzdServer};
pub use config::BOOTSTRAP_CONTRACT_NAMES;
pub mod jstz_rollup_path {
    include!(concat!(env!("OUT_DIR"), "/jstz_rollup_path.rs"));
}
use std::process::exit;

include!("../build_config.rs");
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
pub const JSTZ_NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

/// The `main` function for running jstzd
pub async fn main(config_path: &Option<String>) {
    match config::build_config(config_path).await {
        Ok((port, config)) => run(port, config).await,
        Err(e) => {
            match config_path {
                Some(p) => eprintln!("failed to build config from {}: {:?}", p, e),
                None => eprintln!("failed to build default config: {:?}", e),
            };
            exit(1);
        }
    }
}

async fn run(port: u16, config: JstzdConfig) {
    let mut server = JstzdServer::new(config, port);
    if let Err(e) = server.run().await {
        eprintln!("failed to run jstzd server: {:?}", e);
        let _ = server.stop().await;
        exit(1);
    }

    server.wait().await;

    println!("Shutting down");
    server.stop().await.unwrap();
}
