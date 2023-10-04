mod sandbox;
mod config;
use crate::sandbox::sandbox_start;
use crate::config::Config;

mod deposit;
mod deploy_bridge;
mod deploy_contract;
mod run_contract;
mod utils;
mod sandbox_initializer;

pub fn main(){
    let mut cfg = Config::load_from_file().expect("Failed to load the config.");
    sandbox_start(&mut cfg);
}