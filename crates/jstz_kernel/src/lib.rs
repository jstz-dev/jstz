use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{public_key::PublicKey, smart_function_hash::SmartFunctionHash};
use jstz_proto::{executor, Result};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

use crate::inbox::{read_message, Message};
pub mod inbox;
pub mod parsing;

pub const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
pub const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

fn read_ticketer(rt: &impl Runtime) -> Option<SmartFunctionHash> {
    Storage::get(rt, &TICKETER).ok()?
}

fn read_injector(rt: &impl Runtime) -> Option<PublicKey> {
    Storage::get(rt, &INJECTOR).ok()?
}

async fn handle_message(
    hrt: &mut impl Runtime,
    message: Message,
    ticketer: &ContractKt1Hash,
    tx: &mut Transaction,
    injector: &PublicKey,
) -> Result<()> {
    match message {
        Message::Internal(internal_operation) => {
            let receipt =
                executor::execute_internal_operation(hrt, tx, internal_operation).await;
            receipt.write(hrt, tx)?
        }
        Message::External(signed_operation) => {
            debug_msg!(hrt, "External operation: {signed_operation:?}\n");
            let receipt = executor::execute_operation(
                hrt,
                tx,
                signed_operation,
                ticketer,
                injector,
            );
            debug_msg!(hrt, "Receipt: {receipt:?}\n");
            receipt.write(hrt, tx)?
        }
    }
    Ok(())
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    futures::executor::block_on(async {
        // TODO: we should organize protocol consts into a struct
        // https://linear.app/tezos/issue/JSTZ-459/organize-protocol-consts-into-a-struct
        let ticketer = read_ticketer(rt).expect("Ticketer not found");
        let injector = read_injector(rt).expect("Revealer not found");
        let mut tx = Transaction::default();
        tx.begin();
        if let Some(message) = read_message(rt, &ticketer) {
            handle_message(rt, message, &ticketer, &mut tx, &injector)
                .await
                .unwrap_or_else(|err| debug_msg!(rt, "[🔴] {err:?}\n"));
        }
        if let Err(commit_error) = tx.commit(rt) {
            debug_msg!(rt, "Failed to commit transaction: {commit_error:?}\n");
        }
    })
}

#[cfg(test)]
mod test {

    use jstz_core::kv::Transaction;
    use jstz_crypto::hash::Hash;
    use jstz_mock::{
        host::{JstzMockHost, MOCK_SOURCE},
        message::{fa_deposit::MockFaDeposit, native_deposit::MockNativeDeposit},
    };
    use jstz_proto::{
        context::{
            account::{Account, Address},
            ticket_table::TicketTable,
        },
        executor::smart_function,
        runtime::ParsedCode,
    };
    use tezos_smart_rollup::types::{Contract, PublicKeyHash};

    use crate::{entry, parsing::try_parse_contract, read_ticketer};

    #[test]
    fn read_ticketer_succeeds() {
        let mut host = JstzMockHost::default();
        let ticketer = read_ticketer(host.rt()).unwrap();
        let expected_tickter = host.get_ticketer();
        assert_eq!(ticketer, expected_tickter)
    }

    #[test]
    fn entry_native_deposit_succeeds() {
        let mut host = JstzMockHost::default();
        let deposit = MockNativeDeposit::default();
        host.add_internal_message(&deposit);
        host.rt().run_level(entry);
        let tx = &mut Transaction::default();
        tx.begin();
        match deposit.receiver {
            Contract::Implicit(PublicKeyHash::Ed25519(tz1)) => {
                let amount = Account::balance(
                    host.rt(),
                    tx,
                    &Address::User(jstz_crypto::public_key_hash::PublicKeyHash::Tz1(
                        tz1.into(),
                    )),
                )
                .unwrap();
                assert_eq!(amount, 100);
            }
            _ => panic!("Unexpected receiver"),
        }
    }

    #[test]
    fn entry_fa_deposit_succeeds_with_proxy() {
        let mut host = JstzMockHost::default();

        let tx = &mut Transaction::default();
        tx.begin();
        let parsed_code =
            ParsedCode::try_from(jstz_mock::host::MOCK_PROXY_FUNCTION.to_string())
                .unwrap();
        let addr = Address::User(
            jstz_crypto::public_key_hash::PublicKeyHash::from_base58(MOCK_SOURCE)
                .unwrap(),
        );
        Account::set_balance(host.rt(), tx, &addr, 200).unwrap();
        let proxy =
            smart_function::deploy(host.rt(), tx, &addr, parsed_code, 100).unwrap();
        tx.commit(host.rt()).unwrap();

        let deposit = MockFaDeposit {
            proxy_contract: Some(proxy),
            ..MockFaDeposit::default()
        };

        host.add_internal_message(&deposit);
        host.rt().run_level(entry);
        let ticket_hash = deposit.ticket_hash();
        match deposit.proxy_contract {
            Some(proxy) => {
                tx.begin();
                let proxy_balance = TicketTable::get_balance(
                    host.rt(),
                    tx,
                    &Address::SmartFunction(proxy),
                    &ticket_hash,
                )
                .unwrap();
                assert_eq!(300, proxy_balance);
                let owner = try_parse_contract(&deposit.receiver).unwrap();
                let receiver_balance =
                    TicketTable::get_balance(host.rt(), tx, &owner, &ticket_hash)
                        .unwrap();
                assert_eq!(0, receiver_balance);
            }
            _ => panic!("Unexpected receiver"),
        }
    }

    #[test]
    fn entry_fa_deposit_succeeds_with_invalid_proxy() {
        let mut host = JstzMockHost::default();
        let deposit = MockFaDeposit::default();

        host.add_internal_message(&deposit);
        host.rt().run_level(entry);
        let ticket_hash = deposit.ticket_hash();
        match deposit.proxy_contract {
            Some(proxy) => {
                let mut tx = Transaction::default();
                tx.begin();
                let proxy_balance = TicketTable::get_balance(
                    host.rt(),
                    &mut tx,
                    &Address::SmartFunction(proxy),
                    &ticket_hash,
                )
                .unwrap();
                assert_eq!(0, proxy_balance);
                let owner = try_parse_contract(&deposit.receiver).unwrap();
                let receiver_balance =
                    TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash)
                        .unwrap();
                assert_eq!(300, receiver_balance);
            }
            _ => panic!("Unexpected receiver"),
        }
    }
}
