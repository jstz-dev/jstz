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
        receipt::{DepositReceipt, ReceiptContent, ReceiptResult},
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
            receipt.clone().inner,
            ReceiptResult::Success(ReceiptContent::Deposit(DepositReceipt {
                account,
                updated_balance,
            })) if account == receiver && updated_balance == 20
        ));
        let raw_json_payload = r#"{"hash":[39,12,7,148,87,7,176,168,111,219,214,147,14,123,179,202,232,151,138,59,207,182,101,158,128,98,239,57,236,88,195,42],"inner":{"_type":"Success","inner":{"_type":"DepositReceipt","account":"tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx","updated_balance":20}}}"#;
        assert_eq!(raw_json_payload, serde_json::to_string(&receipt).unwrap());
    }
}
