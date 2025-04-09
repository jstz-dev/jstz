use std::fmt::{self, Display};

use anyhow::Result;
use derive_more::{Deref, DerefMut};
use octez::OctezClient;
use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::BootstrapAccount;

const CTEZ_CONTRACT: &str = include_str!("../../../contracts/jstz_ctez.tz");
const BRIDGE_CONTRACT: &str = include_str!("../../../contracts/jstz_bridge.tz");
const EXCHANGER_CONTRACT: &str = include_str!("../../../contracts/exchanger.tz");
const NATIVE_BRDIGE_CONTRACT: &str =
    include_str!("../../../contracts/jstz_native_bridge.tz");

impl BootstrapAccount {
    fn as_michelson_elt(&self) -> String {
        format!("Elt \"{}\" {}", self.address, self.amount)
    }
}

pub fn deploy_ctez_contract(
    client: &OctezClient,
    operator_address: &str,
    mut bootstrap_accounts: Vec<BootstrapAccount>,
) -> Result<String> {
    bootstrap_accounts.sort();

    let init_storage = format!(
        "(Pair {{ {} }} \"{}\" )",
        bootstrap_accounts
            .iter()
            .map(BootstrapAccount::as_michelson_elt)
            .collect::<Vec<_>>()
            .join(";"),
        operator_address,
    );

    client.originate_contract("jstz_ctez", operator_address, CTEZ_CONTRACT, &init_storage)
}

#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct Exchanger(String);

impl Exchanger {
    pub fn deploy(client: &OctezClient, operator: &str) -> Result<Self> {
        let storage_init = "Unit";
        client
            .originate_contract("exchanger", operator, EXCHANGER_CONTRACT, storage_init)
            .map(Exchanger)
    }
}

impl Display for Exchanger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ContractKt1Hash> for Exchanger {
    fn from(hash: ContractKt1Hash) -> Self {
        Self(hash.to_base58_check())
    }
}

pub struct NativeBridge(String);

impl NativeBridge {
    pub fn deploy(
        client: &OctezClient,
        operator: &str,
        exchanger: &Exchanger,
        rollup_address: &str,
    ) -> Result<NativeBridge> {
        let storage_init =
            format!("(Pair \"{}\" \"{}\" None)", exchanger.0, rollup_address);
        client
            .originate_contract(
                "jstz_native_bridge",
                operator,
                NATIVE_BRDIGE_CONTRACT,
                &storage_init,
            )
            .map(NativeBridge)
    }
}

impl Display for NativeBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct BridgeContract(String);

impl Display for BridgeContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ContractKt1Hash> for BridgeContract {
    fn from(hash: ContractKt1Hash) -> Self {
        Self(hash.to_base58_check())
    }
}

impl BridgeContract {
    pub fn deploy(
        client: &OctezClient,
        operator: &str,
        ctez_address: &str,
    ) -> Result<Self> {
        let init_storage = format!("(Pair None \"{}\" )", ctez_address);

        let bridge_address = client.originate_contract(
            "jstz_bridge",
            operator,
            BRIDGE_CONTRACT,
            &init_storage,
        )?;

        Ok(Self(bridge_address))
    }

    pub fn set_rollup(
        &self,
        client: &OctezClient,
        operator_address: &str,
        rollup_address: &str,
    ) -> Result<()> {
        client.call_contract(
            operator_address,
            &self.0,
            "set_rollup",
            &format!("\"{}\"", rollup_address),
            0.0,
        )
    }
}
