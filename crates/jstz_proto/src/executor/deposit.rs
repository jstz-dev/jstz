use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{context::account::Account, operation::external::Deposit, Result};

pub fn execute<'a, 'b>(
    hrt: &mut impl HostRuntime,
    tx: &'b mut Transaction<'a>,
    deposit: Deposit,
) -> Result<()>
where
    'a: 'b,
{
    let Deposit { amount, reciever } = deposit;

    Account::deposit(hrt, tx, &reciever, amount)
}
