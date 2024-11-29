mod config;
pub mod docker;
pub mod task;
pub use config::BOOTSTRAP_CONTRACT_NAMES;
pub const EXCHANGER_ADDRESS: &str = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
pub const JSTZ_NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

/// The `main` function for running jstzd
pub async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    Ok(())
}
