use clap::Subcommand;
use deploy::DeployBridge;

pub mod deploy;
mod deposit;
mod withdraw;

use crate::{
    config::NetworkName,
    utils::{AddressOrAlias, Tez},
};

use anyhow::Result;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 💰 Deposits XTZ from an existing Tezos L1 address to a jstz address.
    Deposit {
        /// Tezos L1 address or alias to withdraw from (must be stored in octez-client's wallet).
        #[arg(short, long)]
        from: String,
        /// jstz address or alias to deposit to.
        #[arg(short, long)]
        to: AddressOrAlias,
        /// The amount in XTZ to transfer.
        #[arg(short, long)]
        amount: Tez,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// 💰 Withdraws XTZ from the current jstz account to a Tezos L1 address. This command will push
    /// a withdraw outbox message into the jstz outbox which can be executed after the L2 commitment
    /// period has passed to transfer the funds.
    Withdraw {
        /// Tezos L1 address or alias to deposit to (must be stored in octez-client's wallet).
        #[arg(short, long)]
        to: AddressOrAlias,
        /// The amount in XTZ to transfer.
        #[arg(short, long)]
        amount: Tez,
        /// Specifies the network from the config file, defaulting to the configured default network.
        /// Use `dev` for the local sandbox.
        #[arg(short, long, default_value = None)]
        network: Option<NetworkName>,
    },
    /// Deploys an FA token bridge with minimal functionality.
    /// Given a valid a valid L1 FA token contract and jstz token smart function, it will deploy the corresponding L1 bridge and ticket contracts.
    ///
    /// For example, the flow of depositing a FA token via the bridge involves:
    /// 1. Approving the FA token contract to transfer tokens from the depositor to the bridge by calling the `update_operators` function.
    /// 2. Transferring the FA token to the bridge contract.
    /// 3. Locking the FA token and minting the same amount of tickets to the bridge contract.
    /// 4. The tickets are then sent to the L2 and can be redeemed for the FA token on the L2.
    FaDeploy(DeployBridge),
}

pub async fn exec(command: Command) -> Result<()> {
    match command {
        Command::Deposit {
            from,
            to,
            amount,
            network,
        } => deposit::exec(from, to, amount, network).await,
        Command::Withdraw {
            to,
            amount,
            network,
        } => withdraw::exec(to, amount, network).await,
        Command::FaDeploy(deploy) => {
            let _ = deploy.exec().await?;
            Ok(())
        }
    }
}
