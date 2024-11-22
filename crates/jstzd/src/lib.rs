mod config;
pub mod docker;
pub mod task;

include!("../build_config.rs");
pub const JSTZ_ROLLUP_ADDRESS: &str = "sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK";
pub const JSTZ_NATIVE_BRIDGE_ADDRESS: &str = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

/// The `main` function for running jstzd
pub async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    Ok(())
}
