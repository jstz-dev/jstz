use std::path::Path;

use anyhow::Context;
use jstz_utils::inbox_builder::InboxBuilder;

use clap::Parser;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::types::SmartRollupAddress;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Target rollup address.
    #[arg(long)]
    rollup_address: String,

    /// Jstz ticketer contract address.
    #[arg(long)]
    ticketer_address: String,

    /// Path to the output inbox file.
    #[arg(long, default_value = "inbox.json")]
    inbox_file: Box<Path>,
}

/// Generates inbox messages for most of the operations available in Jstz.
fn main() -> jstz_tps_bench::Result<()> {
    let args = Args::parse();
    let rollup_addr = SmartRollupAddress::from_b58check(&args.rollup_address)
        .context("failed to parse rollup address")?;
    let ticketer_addr = ContractKt1Hash::from_base58_check(&args.ticketer_address)
        .context("failed to parse ticketer address")?;
    let mut builder = InboxBuilder::new(rollup_addr, Some(ticketer_addr));
    let mut accounts = builder.create_accounts(2)?;

    builder.deposit_from_l1(&mut accounts[0], 1000000)?;

    let inbox = builder.build();
    inbox.save(&args.inbox_file)
}
