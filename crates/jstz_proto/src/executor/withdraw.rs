use jstz_core::{
    host::HostRuntime,
    kv::{outbox::OutboxMessage, Transaction},
};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::{
    michelson::{ticket::FA2_1Ticket, MichelsonOption, MichelsonPair},
    types::Contract,
};

use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::{
    context::new_account::{Account, Addressable, Amount, NewAddress},
    Error, Result,
};

const BURN_ENTRYPOINT: &str = "burn";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Withdrawal {
    pub amount: Amount,
    pub receiver: NewAddress,
}

fn create_withdrawal(
    amount: Amount,
    receiver: &NewAddress,
    ticketer: &ContractKt1Hash,
) -> Result<OutboxMessage> {
    let receiver_pkh = receiver.to_base58();
    let ticket = FA2_1Ticket::new(
        Contract::Originated(ticketer.clone()),
        MichelsonPair(0.into(), MichelsonOption(None)),
        amount,
    )
    .map_err(|_| Error::InvalidTicketType)?;
    let message = OutboxMessage::new_withdrawal_message(
        &Contract::try_from(receiver_pkh).unwrap(),
        &Contract::Originated(ticketer.clone()),
        ticket,
        BURN_ENTRYPOINT,
    )?;
    Ok(message)
}

fn withdraw(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &impl Addressable,
    withdrawal: Withdrawal,
    ticketer: &ContractKt1Hash,
) -> Result<()> {
    let Withdrawal { amount, receiver } = withdrawal;
    Account::sub_balance(rt, tx, source, amount)?;
    let message = create_withdrawal(amount, &receiver, ticketer)?;
    tx.queue_outbox_message(rt, message)?;
    Ok(())
}

/// Process the native withdrawal request by atomically deducting user balance
/// then pushing a withdraw message to the outbox queue.`ticketer` is expected
/// to be the XTZ Exchanger contract. See /jstz/contracts/exchanger.mligo.
///
/// Fails if the source account has insufficient funds or if the outbox
/// queue is full.
pub(crate) fn execute_withdraw(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &impl Addressable,
    withdrawal: Withdrawal,
    ticketer: &ContractKt1Hash,
) -> Result<()> {
    tx.begin();
    let result = withdraw(rt, tx, source, withdrawal, ticketer);
    if result.is_ok() {
        tx.commit(rt)?;
    } else {
        tx.rollback()?;
    }
    result
}

#[cfg(test)]
mod test {
    use jstz_core::kv::Transaction;
    use jstz_mock::{self};
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{new_account::Account, new_account::NewAddress},
        executor::withdraw::execute_withdraw,
        Error,
    };

    use super::Withdrawal;

    #[test]
    fn execute_withdraw_fails_on_insufficient_funds() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = NewAddress::User(jstz_mock::account1());
        let withdrawal = Withdrawal {
            amount: 11,
            receiver: NewAddress::User(jstz_mock::account2()),
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        tx.begin();
        Account::add_balance(&host, &mut tx, &source, 10)
            .expect("Add balance should succeed");
        tx.commit(&mut host).unwrap();

        tx.begin();
        let result = execute_withdraw(&mut host, &mut tx, &source, withdrawal, &ticketer);
        assert!(matches!(result, Err(Error::InsufficientFunds)));

        assert_eq!(10, Account::balance(&host, &mut tx, &source).unwrap());
        let level = host.run_level(|_| {});
        assert_eq!(0, host.outbox_at(level).len());
    }

    #[test]
    fn execute_withdraw_succeeds() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = NewAddress::User(jstz_mock::account1());
        let withdrawal = Withdrawal {
            amount: 10,
            receiver: NewAddress::User(jstz_mock::account2()),
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();

        tx.begin();
        Account::add_balance(&host, &mut tx, &source, 10)
            .expect("Add balance should succeed");
        tx.commit(&mut host).unwrap();

        tx.begin();
        execute_withdraw(&mut host, &mut tx, &source, withdrawal, &ticketer).unwrap();

        tx.commit(&mut host).unwrap();
        let level = host.run_level(|_| {});
        assert_eq!(1, host.outbox_at(level).len());

        tx.begin();
        let balance = Account::balance(&host, &mut tx, &source).unwrap();
        assert_eq!(0, balance)
    }
}
