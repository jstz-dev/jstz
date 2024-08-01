use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::types::{PublicKeyHash, SmartRollupAddress};

pub mod fa_deposit;
pub mod native_deposit;

pub trait MockInternalMessage {
    fn source(&self) -> PublicKeyHash;
    fn sender(&self) -> ContractKt1Hash;
    fn smart_rollup(&self) -> Option<SmartRollupAddress>;
}
