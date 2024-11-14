use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::hash::Blake2b;

use crate::{
    context::account::Account,
    operation::external::Deposit,
    receipt::{DepositReceipt, Receipt},
};

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: Deposit,
) -> Receipt {
    let Deposit {
        amount, receiver, ..
    } = deposit;

    let result = Account::add_balance(hrt, tx, &receiver, amount);
    let hash = Blake2b::from(deposit.inbox_id.to_be_bytes().as_slice());
    Receipt::new(
        hash,
        result.map(|updated_balance| {
            crate::receipt::ReceiptContent::Deposit(DepositReceipt {
                account: receiver,
                updated_balance,
            })
        }),
    )
}

#[cfg(test)]
mod test {
    use jstz_core::kv::Transaction;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        operation::external::Deposit,
        receipt::{DepositReceipt, ReceiptContent},
    };

    use super::execute;

    #[test]
    fn test_execute_receipt() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let receiver = jstz_mock::account1();
        let deposit = Deposit {
            inbox_id: 1,
            amount: 20,
            receiver: receiver.clone(),
        };
        tx.begin();
        let receipt = execute(&mut host, &mut tx, deposit);
        assert!(matches!(
            receipt.inner,
            Ok(ReceiptContent::Deposit(DepositReceipt {
                account,
                updated_balance,
            })) if account == receiver && updated_balance == 20
        ))
    }
}
