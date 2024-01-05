use anyhow::Result;
use clap::Subcommand;

mod deposit;

use crate::config::Config;

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
