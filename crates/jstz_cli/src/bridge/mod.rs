use clap::Subcommand;

mod deposit;

use crate::{error::Result, utils::AddressOrAlias};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 💰 Deposits from an existing Tezos L1 address to a jstz address.
    Deposit {
        /// Tezos L1 address or alias to withdraw from (must be stored in octez-client's wallet).
        #[arg(short, long)]
        from: String,
        /// jstz address or alias to deposit to.
        #[arg(short, long)]
        to: AddressOrAlias,
        /// The amount in CTEZ to transfer.
        #[arg(short, long)]
        amount: u64,
    },
}

pub fn exec(command: Command) -> Result<()> {
    match command {
        Command::Deposit { from, to, amount } => deposit::exec(from, to, amount),
    }
}
