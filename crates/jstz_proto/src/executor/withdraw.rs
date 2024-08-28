use jstz_core::{
    host::HostRuntime,
    kv::{outbox::OutboxMessage, Transaction},
};
use serde::Deserialize;
use tezos_smart_rollup::{
    michelson::{
        ticket::FA2_1Ticket, MichelsonContract, MichelsonNat, MichelsonOption,
        MichelsonPair,
    },
    outbox::OutboxMessageTransaction,
    types::{Contract, Entrypoint},
};

use tezos_crypto_rs::hash::ContractKt1Hash;

use crate::{
    context::account::{Account, Address, Amount},
    Result,
};

const BURN_ENTRYPOINT: &str = "burn";

#[derive(Debug, Deserialize)]
pub struct Withdrawal {
    pub amount: Amount,
    pub receiver: Address,
}

fn create_withdrawal(
    amount: Amount,
    receiver: &Address,
    ticketer: &ContractKt1Hash,
) -> Result<OutboxMessage> {
    let pkh = receiver.to_base58();
    let entrypoint = Entrypoint::try_from(BURN_ENTRYPOINT.to_string()).unwrap();
    let parameters = MichelsonPair(
        MichelsonContract(Contract::try_from(pkh).unwrap()),
        FA2_1Ticket::new(
            Contract::Originated(ticketer.clone()),
            MichelsonPair(MichelsonNat::from(0), MichelsonOption(None)),
            amount,
        )
        .unwrap(),
    );
    let message = OutboxMessage::Withdrawal(OutboxMessageTransaction {
        entrypoint,
        parameters,
        destination: Contract::Originated(ticketer.clone()),
    });
    Ok(message)
}

pub fn native_withdraw(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &Address,
    withdrawal: Withdrawal,
    ticketer: &ContractKt1Hash,
) -> Result<()> {
    let Withdrawal { amount, receiver } = withdrawal;
    Account::sub_balance(rt, tx, source, amount)?;
    let message = create_withdrawal(amount, &receiver, ticketer)?;
    tx.push_outbox_message(message)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use jstz_core::kv::Transaction;
    use jstz_mock::{self, host::JstzMockHost};

    use crate::{
        context::account::Account, executor::deposit, operation::external::Deposit,
    };

    use super::Withdrawal;

    #[test]
    fn execute_native_withdraw() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let source = jstz_mock::account1();
        let withdrawal = Withdrawal {
            amount: 10,
            receiver: jstz_mock::account2(),
        };
        let ticketer = host.get_ticketer();
        let hrt = host.rt();

        deposit::execute(
            hrt,
            &mut tx,
            Deposit {
                inbox_id: 1,
                amount: 10000,
                receiver: source.clone(),
            },
        );

        super::native_withdraw(hrt, &mut tx, &source, withdrawal, &ticketer).unwrap();

        tx.commit(hrt).unwrap();

        let level = hrt.run_level(|_| {});
        let outbox = hrt.outbox_at(level);

        assert_eq!(1, outbox.len());

        tx.begin();

        let balance = Account::balance(hrt, &mut tx, &source).unwrap();
        assert_eq!(9990, balance)
    }
}
