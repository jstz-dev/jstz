mod config;
pub mod docker;
pub mod task;
mod user_config;

use crate::task::jstzd::{JstzdConfig, JstzdServer};
pub use config::BOOTSTRAP_CONTRACT_NAMES;
pub mod jstz_rollup_path {
    include!(concat!(env!("OUT_DIR"), "/jstz_rollup_path.rs"));

    pub fn riscv_kernel_descriptor() -> String {
        let kernel_path = riscv_kernel_path();
        let kernel_checksum = riscv_kernel_checksum();
        format!("kernel:{}:{}", kernel_path.display(), kernel_checksum)
    }
}
use console::style;
use std::io::{stdout, Write};
use std::process::exit;
use tokio::signal::unix::{signal, SignalKind};

pub use config::*;

include!("../build_config.rs");
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
pub const JSTZ_NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";
const JSTZ_BANNER: &str = r#"
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
    match config::build_config_from_path(config_path).await {
        Ok((port, config)) => run(port, config).await,
        Err(e) => {
            match config_path {
                Some(p) => eprintln!("failed to build config from {p}: {e:?}"),
                None => eprintln!("failed to build default config: {e:?}"),
            };
            exit(1);
        }
    }
}

// requiring a writer here so that we can test this function
fn print_banner(writer: &mut impl Write) {
    let _ = writeln!(writer, "{}", style(JSTZ_BANNER).bold());
    let _ = writeln!(
        writer,
        "        {} {}",
        env!("CARGO_PKG_VERSION"),
        style(env!("CARGO_PKG_REPOSITORY")).blue().bold()
    );
    let _ = writeln!(writer);
}

async fn run(port: u16, config: JstzdConfig) {
    let mut server = JstzdServer::new(config, port);
    print_banner(&mut stdout());
    if let Err(e) = server.run(true).await {
        eprintln!("failed to run jstzd server: {e:?}");
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

#[cfg(test)]
mod lib_test {
    #[test]
    fn print_banner() {
        let mut buf = vec![];
        super::print_banner(&mut buf);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains(super::JSTZ_BANNER));
        assert!(s.contains(env!("CARGO_PKG_VERSION")));
        assert!(s.contains(env!("CARGO_PKG_REPOSITORY")));
    }
}
