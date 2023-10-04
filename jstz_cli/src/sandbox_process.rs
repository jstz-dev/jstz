mod config;
mod sandbox;
use config::Config;
use sandbox::sandbox_start;

mod deploy_bridge;
mod deploy_contract;
mod deposit;
mod run_contract;
mod sandbox_initializer;
mod utils;

pub fn main() {
    let mut cfg = Config::load_from_file().expect("Failed to load the config.");
    sandbox_start(&mut cfg);
}
