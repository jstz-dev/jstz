use jstz_core::{host::HostRuntime, kv::Transaction};

use crate::{
    context::account::Account,
    operation::internal::Deposit,
    receipt::{DepositReceipt, Receipt},
};

pub fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: Deposit,
) -> Receipt {
    let hash = deposit.hash();
    let result = Account::add_balance(hrt, tx, &deposit.receiver, deposit.amount);

    Receipt::new(
        hash,
        result.map(|updated_balance| {
            crate::receipt::ReceiptContent::Deposit(DepositReceipt {
                account: deposit.receiver,
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
        context::account::Address,
        operation::internal::{Deposit, InboxId},
        receipt::{DepositReceipt, ReceiptContent, ReceiptResult},
    };

    use super::execute;

    #[test]
    fn test_execute_receipt() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let receiver = jstz_mock::account1();
        let deposit = Deposit {
            inbox_id: InboxId {
                l1_level: 1,
                l1_message_id: 1,
            },
            amount: 20,
            receiver: Address::User(receiver.clone()),
        };
        tx.begin();
        let receipt = execute(&mut host, &mut tx, deposit);
        assert!(matches!(
            receipt.clone().result,
            ReceiptResult::Success(ReceiptContent::Deposit(DepositReceipt {
                account,
                updated_balance,
            })) if account == Address::User(receiver) && updated_balance == 20
        ));
        let raw_json_payload = r#"{"hash":[206,213,136,201,149,142,122,49,150,82,57,204,46,196,114,184,123,37,238,1,24,101,217,191,25,104,254,193,105,5,238,188],"result":{"_type":"Success","inner":{"_type":"Deposit","account":"tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx","updatedBalance":20}}}"#;
        assert_eq!(raw_json_payload, serde_json::to_string(&receipt).unwrap());
    }
}
