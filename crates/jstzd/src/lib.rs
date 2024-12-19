mod config;
pub mod docker;
pub mod task;

use crate::task::jstzd::{JstzdConfig, JstzdServer};
pub use config::BOOTSTRAP_CONTRACT_NAMES;
pub mod jstz_rollup_path {
    include!(concat!(env!("OUT_DIR"), "/jstz_rollup_path.rs"));
}
use console::style;
use std::process::exit;
use tokio::signal::unix::{signal, SignalKind};

include!("../build_config.rs");
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
pub const JSTZ_NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";
const SANDBOX_BANNER: &str = r#"
           __________
           \  jstz  /
            )______(
            |""""""|_.-._,.---------.,_.-._
            |      | | |               | | ''-.
            |      |_| |_             _| |_..-'
            |______| '-' `'---------'` '-'
            )""""""(
           /________\
           `'------'`
         .------------.
        /______________\
"#;

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

fn print_banner() {
    println!("{}", style(SANDBOX_BANNER).bold());
    println!(
        "        {} {}",
        env!("CARGO_PKG_VERSION"),
        style(env!("CARGO_PKG_REPOSITORY")).blue().bold()
    );
    println!();
}

async fn run(port: u16, config: JstzdConfig) {
    let mut server = JstzdServer::new(config, port);
    print_banner();
    if let Err(e) = server.run(true).await {
        eprintln!("failed to run jstzd server: {:?}", e);
        let _ = server.stop().await;
        exit(1);
    }

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = server.wait() => (),
        _ = sigterm.recv() => (),
        _ = sigint.recv() => (),
    };
    println!("Shutting down");
    server.stop().await.unwrap();
}
