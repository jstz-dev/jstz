use anyhow::Result;
use clap::Subcommand;

mod deposit;

use crate::config::Config;

const BOOTSTRAP3_ADDRESS: &str = "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU";

pub fn deploy(cfg: &Config) -> Result<String> {
    let bridge_dir = cfg.jstz_path.join("contracts");

    // 1. Originate ctez contract
    let init_ctez_storage = format!(
        "(Pair \"{}\" {{ Elt \"{}\" 10000000000 }} )",
        BOOTSTRAP3_ADDRESS, BOOTSTRAP3_ADDRESS
    );

    let ctez_address = cfg.octez_client()?.originate_contract(
        "jstz_ctez",
        "bootstrap3",
        bridge_dir
            .join("jstz_ctez.tz")
            .to_str()
            .expect("Invalid path"),
        &init_ctez_storage,
    )?;

    // 2. Originate bridge contract
    let init_bridge_storage = format!("(Pair \"{}\" None)", ctez_address);

    let bridge_address = cfg.octez_client()?.originate_contract(
        "jstz_bridge",
        "bootstrap3",
        bridge_dir
            .join("jstz_bridge.tz")
            .to_str()
            .expect("Invalid path"),
        &init_bridge_storage,
    )?;

    Ok(bridge_address)
}

pub fn set_rollup(cfg: &Config, rollup_address: &str) -> Result<()> {
    cfg.octez_client()?.call_contract(
        "bootstrap3",
        "jstz_bridge",
        "set_rollup",
        &format!("\"{}\"", rollup_address),
    )
}

#[derive(Subcommand)]
pub enum Command {
    /// Deposits from an existing L1 sandbox address to a L2 sandbox address.
    Deposit {
        /// The L1 sandbox address or alias to withdraw from.
        #[arg(short, long)]
        from: String,
        /// The L2 sandbox address or alias to deposit to.
        #[arg(short, long)]
        to: String,
        /// The amount in ctez to transfer.
        #[arg(short, long)]
        amount: u64,
    },
}

pub fn exec(command: Command, cfg: &Config) -> Result<()> {
    match command {
        Command::Deposit { from, to, amount } => deposit::exec(from, to, amount, cfg),
    }
}
