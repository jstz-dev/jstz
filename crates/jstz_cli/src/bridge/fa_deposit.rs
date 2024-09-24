use anyhow::Result;
use clap::Args;
use log::info;

use crate::{
    config::{Config, NetworkName},
    error::bail_user_error,
    utils::{AddressOrAlias, OriginatedOrAlias},
};

#[derive(Debug, Args)]
pub struct FaDeposit {
    /// Sender address or alias that will pay L1 gas fees and
    /// have its FA token account debited
    #[arg()]
    sender: AddressOrAlias,
    /// Receiver address in Jstz that will be credited by the
    /// the FA smart function. Defaults to the current account
    #[arg(short, long)]
    receiver: Option<AddressOrAlias>,
    /// Token bridge address or alias
    #[arg()]
    fa_token_bridge: OriginatedOrAlias,
    /// Number of tokens to deposit
    #[arg()]
    amount: u32,
    /// Specifies the network from the config file, defaulting to the configured default network.
    /// Use `dev` for the local sandbox.
    #[arg(short, long, default_value = None)]
    network: Option<NetworkName>,
}

impl FaDeposit {
    pub fn exec(self) -> Result<()> {
        let FaDeposit {
            sender,
            receiver,
            fa_token_bridge,
            amount,
            network,
        } = self;
        let cfg = Config::load()?;
        let client = cfg.octez_client(&network)?;
        let receiver = match receiver {
            Some(r) => r,
            None => {
                if let Some((_, user)) = cfg.accounts.current_user() {
                    AddressOrAlias::Address(user.address.clone())
                } else {
                    bail_user_error!(
                        "You are not logged in. Please add -r <alias> to select \
                         the receiver account or run `jstz login <alias>` and try again."
                    )
                }
            }
        };

        let sender_address = sender.resolve_l1(&cfg, &network)?;
        let fa_bridge_address = fa_token_bridge.resolve_l1(&cfg, &network)?;
        let receiver_address = receiver.resolve(&cfg)?;
        let jstz_address = client.resolve_jstz_addres("jstz_rollup")?;

        let parameter = format!(
            "Pair \"{}\" \"{}\" {}",
            jstz_address, receiver_address, amount
        );

        client.call_contract(
            sender_address.to_base58().as_str(),
            fa_bridge_address.to_base58_check().as_str(),
            "deposit",
            parameter.as_str(),
            0,
        )?;

        info!("Bridge deposit injected");

        Ok(())
    }
}
