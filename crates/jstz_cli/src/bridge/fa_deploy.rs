use clap::{arg, Args};
use log::info;
use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::{
    config::{Config, NetworkName},
    utils::{AddressOrAlias, OriginatedOrAlias},
};

use anyhow::Result;

const FA_TICKETER: &str =
    include_str!("../../../../contracts/examples/fa_ticketer/fa_ticketer.tz");
const JSTZ_FA_BRIDGE: &str = include_str!("../../../../contracts/jstz_fa_bridge.tz");

enum FaToken {
    Fa12 {
        address: ContractKt1Hash,
    },
    Fa2 {
        address: ContractKt1Hash,
        token_id: u32,
    },
}

impl FaToken {
    pub fn from(address: &ContractKt1Hash, token_id: Option<u32>) -> FaToken {
        match token_id {
            Some(token_id) => FaToken::Fa2 {
                address: address.clone(),
                token_id,
            },
            None => FaToken::Fa12 {
                address: address.clone(),
            },
        }
    }

    pub fn to_micheline(&self) -> String {
        match self {
            FaToken::Fa12 { address } => format!("Left \"{}\"", address),
            FaToken::Fa2 { address, token_id } => {
                format!("Right (Pair \"{}\" {})", address, token_id)
            }
        }
    }
}

fn format_ticket_content(ticket_id: u32, content: Option<String>) -> Result<String> {
    let content = match content {
        Some(value) => {
            let _: serde::de::IgnoredAny = serde_json::from_str(value.as_str())?;
            let bytes = hex::encode(value);
            anyhow::Ok(format!("Some 0x{}", bytes))
        }
        None => Ok("None".to_string()),
    }?;
    Ok(format!("Pair {} {}", ticket_id, content))
}

#[derive(Debug, Args)]
pub struct FaDeploy {
    /// Source address or alias that will pay L1 gas fees
    #[arg()]
    source: AddressOrAlias,
    /// Ticket id for newly minted tickets
    #[arg()]
    ticket_id: u32,
    /// Ticket content for newly minted tickets
    #[arg(long)]
    ticket_content: Option<String>,
    /// Total ticket supply
    #[arg()]
    total_ticket_supply: u32,
    /// Tezos L1 address or alias (must be stored in octez-client's wallet) of the FA token contract.
    /// Can be either an FA1.2 or FA2 contract
    #[arg()]
    tezos_fa_token: OriginatedOrAlias,
    /// Token id if the token is an FA2 token
    #[arg(long = "token-id")]
    fa_token_id: Option<u32>,
    /// Jstz address or alias of the FA smart function
    #[arg()]
    jstz_fa_token: AddressOrAlias,
    /// Specifies the network from the config file, defaulting to the configured default network.
    /// Use `dev` for the local sandbox.
    #[arg(short, long, default_value = None)]
    network: Option<NetworkName>,
}

impl FaDeploy {
    pub fn exec(self) -> Result<()> {
        let FaDeploy {
            source,
            tezos_fa_token,
            fa_token_id,
            jstz_fa_token,
            ticket_id,
            ticket_content,
            total_ticket_supply,
            network,
        } = self;
        let cfg = Config::load()?;
        let client = cfg.octez_client(&network)?;

        let source = source.resolve_l1(&cfg, &network)?;
        let jstz_fa_token_address = jstz_fa_token.resolve(&cfg)?;
        let fa_token_address = tezos_fa_token.resolve_l1(&cfg, &network)?;
        let fa_token_object = FaToken::from(&fa_token_address, fa_token_id);

        let ticketer_name = format!("{}-ticketer", tezos_fa_token.to_string());
        let ticketer_storage = format!(
            "Pair {{}} ({}) ({}) {}",
            fa_token_object.to_micheline(),
            format_ticket_content(ticket_id, ticket_content)?,
            total_ticket_supply
        );

        let ticketer_address = client.originate_contract(
            ticketer_name.as_str(),
            &source.to_base58(),
            FA_TICKETER,
            &ticketer_storage,
        )?;

        info!(
            "FA Ticketer (alias: {}) deployed at address {}",
            ticketer_name, ticketer_address
        );

        let bridge_name = format!("{}-bridge", tezos_fa_token.to_string());
        let bridge_storage = format!(
            "Pair ({}) \"{}\" (Some \"{}\") None {{}}",
            fa_token_object.to_micheline(),
            ticketer_address,
            jstz_fa_token_address
        );
        let bridge_address = client.originate_contract(
            bridge_name.as_str(),
            &source.to_base58(),
            JSTZ_FA_BRIDGE,
            &bridge_storage,
        )?;

        info!(
            "FA bridge (alias: {}) deployed at address {}",
            bridge_name, bridge_address
        );

        Ok(())
    }
}