use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::hash::Blake2b;

use crate::{context::account::Account, operation::external::Deposit, receipt::Receipt};

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: Deposit,
) -> Receipt {
    let Deposit {
        amount, receiver, ..
    } = deposit;

    // FIXME: If The result fails due to overflow, we need to push an tez
    // withdraw message to the outbox. In practice, the bridge should check
    // that the amount is valid.
    let result = Account::add_balance(hrt, tx, &receiver, amount);
    let hash = Blake2b::from(deposit.inbox_id.to_be_bytes().as_slice());
    Receipt::new(hash, result.map(|_| crate::receipt::Content::Deposit))
}
