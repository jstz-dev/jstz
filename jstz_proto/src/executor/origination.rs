use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::public_key_hash::PublicKeyHash;

use crate::{
    context::account::Account, operation::external::ContractOrigination, Result,
};

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    contract: ContractOrigination,
) -> Result<PublicKeyHash> {
    let ContractOrigination {
        originating_address,
        contract_code,
        initial_balance,
    } = contract;
    let nonce = Account::nonce(hrt, tx, &originating_address)?;
    let contract_address = PublicKeyHash::digest(
        format!(
            "{}{}{}",
            originating_address.to_string(),
            contract_code.to_string(),
            nonce.to_string(),
        )
        .as_bytes(),
    )?;
    Account::create(
        hrt,
        tx,
        &contract_address,
        initial_balance,
        Some(contract_code),
    )?;
    Ok(contract_address)
}
