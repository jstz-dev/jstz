mod config;
pub mod docker;
pub mod task;

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
        Ok((_port, _config)) => {
            // TODO: run JstzdServer here
            println!("ready");
        }
        Err(e) => {
            match config_path {
                Some(p) => eprintln!("failed to build config from {}: {:?}", p, e),
                None => eprintln!("failed to build default config: {:?}", e),
            };
            exit(1);
        }
    }
}
