use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{context::account::Account, operation::external::Deposit, Result};

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: Deposit,
) -> Result<()> {
    let Deposit { amount, reciever } = deposit;

    Account::deposit(hrt, tx, &reciever, amount)
}
