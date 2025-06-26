use std::sync::Arc;

use jstz_core::{host::JsHostRuntime, kv::Transaction};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::prelude::{debug_msg, Runtime};

use crate::{
    handle_message,
    inbox::{self, ParsedInboxMessage},
    read_injector, read_ticketer,
};

/// Runs the event loop within LocalSet which maintains a task FIFO queue. This is
/// desirable because there is an expectation within blockchains to process operations
/// in input order. Unfortunately, tokio doesn't give granular control to enforce priority
/// queuing
///
/// Additionally, LocalSet supports support `!Send` futures which is currently required
/// by [`JsHostRuntime`]
pub fn run(rt: &mut impl Runtime) {
    let tokio_runtime = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            debug_msg!(rt, "Failed to build Tokio runtime: {:?}", e);
            return;
        }
    };
    let local_set = tokio::task::LocalSet::new();
    local_set.block_on(&tokio_runtime, run_event_loop(rt))
}

/// Jstz kernel event Loop
///
/// On each iteration, read a message, spawn a local task for handling that message
/// then yield to the scheduler. Yielding re-adds the current task to the back of
/// the executor task queue
async fn run_event_loop(rt: &mut impl Runtime) {
    let ticketer = Arc::new(read_ticketer(rt));
    let injector = Arc::new(read_injector(rt));

    loop {
        match read_message(rt, &ticketer) {
            Some(ParsedInboxMessage::JstzMessage(message)) => {
                let ticketer = ticketer.clone();
                let injector = injector.clone();
                let mut host = JsHostRuntime::new(rt);
                // SpawnError only occurs in spawn_local when the executor has shutdown
                tokio::task::spawn_local(async move {
                    let mut tx = Transaction::default();
                    tx.begin();
                    handle_message(&mut host, message, &ticketer, &mut tx, &injector)
                        .await
                        .unwrap_or_else(|err| debug_msg!(&host, "[🔴] {err:?}\n"));
                    if let Err(commit_error) = tx.commit(&mut host) {
                        debug_msg!(
                            &host,
                            "Failed to commit transaction: {commit_error:?}\n"
                        );
                    }
                });
            }
            Some(ParsedInboxMessage::LevelInfo(_)) => {}
            None => {
                // See `read_message` for cases that return None
                // Break enabled in tests only
                #[cfg(test)]
                break;
            }
        }
        // Yields twice; Once for processing the new task, the second for
        // processing tasks that were awaken by the first.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
    }
}

// We reach None in 3 cases
// 1. No more inputs
// 2. Input targetting the wrong rollup
// 3. Parsing failures
fn read_message(
    rt: &mut impl Runtime,
    ticketer: &ContractKt1Hash,
) -> Option<ParsedInboxMessage> {
    let input = rt.read_input().ok()??;
    let jstz_rollup_address = rt.reveal_metadata().address();
    inbox::parse_inbox_message(
        rt,
        input.id,
        input.as_ref(),
        ticketer,
        &jstz_rollup_address,
    )
}

#[cfg(test)]
mod test {

    use jstz_core::kv::Transaction;
    use jstz_crypto::{
        hash::Hash, public_key::PublicKey, public_key_hash::PublicKeyHash,
        secret_key::SecretKey,
    };
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
        operation::{DeployFunction, Operation, RunFunction, SignedOperation},
        runtime::ParsedCode,
    };
    use tezos_smart_rollup::types::{
        Contract as L1Address, PublicKeyHash as L1PublicKeyHash,
    };

    use crate::{parsing::try_parse_contract, read_ticketer};

    use super::run;

    fn set_transfer_header(run_func: &mut RunFunction, amount: u64) {
        run_func
            .headers
            .insert("X-JSTZ-TRANSFER", amount.try_into().unwrap());
    }

    /*
       Scenario
       - op1: 100 mutez deposited into bobs account
       - op2: bob transfers 30 mutez to alice
       - op3: alice deploys SF that forwards mutez to bob
       - op4: alice runs the SF, send sending 10  mutez

       Check
       - bob has 80 mutez
       - alice has 20 mutez
    */
    #[test]
    fn scenario_1() -> Result<(), anyhow::Error> {
        let mut host = JstzMockHost::new(false);
        // host.set_debug_handler(std::io::stdout());
        let bob_sk = SecretKey::from_base58(
            "edsk3eA4FyZDnDSC2pzEh4kwnaLLknvdikvRuXZAV4T4pWMVd6GUyS",
        )?;
        let bob_pk = PublicKey::from_base58(
            "edpkusQcxu7Zv33x1p54p62UgzcawjBRSdEFJbPKEtjQ1h1TaFV3U5",
        )?;

        let alice_sk = SecretKey::from_base58(
            "edsk38mmuJeEfSYGiwLE1qHr16BPYKMT5Gg1mULT7dNUtg3ti4De3a",
        )?;
        let alice_pk = PublicKey::from_base58(
            "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
        )?;

        // 100 mutez deposited into bob's account
        let op1 = MockNativeDeposit::new(
            100,
            None,
            Some(L1Address::from_b58check(bob_pk.hash().as_str())?),
        );

        // bob transfers 30 mutez to alice
        let op2 = {
            let mut run_fn = RunFunction {
                uri: format!("jstz://{}", alice_pk.hash()).parse()?,
                method: http::Method::GET,
                headers: http::HeaderMap::new(),
                body: None,
                gas_limit: 0,
            };
            set_transfer_header(&mut run_fn, 30);
            let op = Operation {
                public_key: bob_pk.clone(),
                nonce: 0.into(),
                content: run_fn.into(),
            };
            let sig = bob_sk.sign(op.hash())?;
            SignedOperation::new(sig, op)
        };

        // alice deploys sf that forwards tez to bob
        let op3 = {
            let code = format!(
                r#"
            export default async (request) => {{
                // forwards mutez to bob
                let amount = request.headers.get("x-jstz-amount");
                let resp = await fetch("jstz://{}", {{
                    headers: {{ "x-jstz-transfer": amount }}
                }});
                return resp
            }}
            "#,
                bob_pk.hash()
            );
            let deploy_fn = DeployFunction {
                function_code: ParsedCode::try_from(code).unwrap(),
                account_credit: 0,
            };
            let op = Operation {
                public_key: alice_pk.clone(),
                nonce: 0.into(),
                content: deploy_fn.into(),
            };
            let sig = alice_sk.sign(op.hash())?;
            SignedOperation::new(sig, op)
        };

        // alice runs previously deployed sf
        let op4 = {
            let mut run_fn = RunFunction {
                uri: "jstz://KT1EPRuE9JnmkJFw58W39hBoiCmX14XtMgGd".parse()?,
                method: http::Method::GET,
                headers: http::HeaderMap::new(),
                body: None,
                gas_limit: 0,
            };
            set_transfer_header(&mut run_fn, 10);
            let op = Operation {
                public_key: alice_pk.clone(),
                nonce: 1.into(),
                content: run_fn.into(),
            };
            let sig = alice_sk.sign(op.hash())?;
            SignedOperation::new(sig, op)
        };

        // Add operations to inbox and run
        host.add_internal_message(&op1);
        host.add_external_message(op2);
        host.add_external_message(op3.clone());
        host.add_external_message(op4.clone());

        // Will exist when out of inbox message only in tests.
        host.run_level(run);

        // // Validated balances
        let mut tx = Transaction::default();
        tx.begin();
        let bob_balance = Account::balance(
            &mut *host,
            &mut tx,
            &PublicKeyHash::from_base58(bob_pk.hash().as_str())?,
        )
        .unwrap();
        let alice_balance = Account::balance(
            &mut *host,
            &mut tx,
            &PublicKeyHash::from_base58(alice_pk.hash().as_str())?,
        )
        .unwrap();

        assert_eq!(80, bob_balance);
        assert_eq!(20, alice_balance);

        Ok(())
    }

    #[test]
    fn read_ticketer_succeeds() {
        let mut host = JstzMockHost::default();
        let ticketer = read_ticketer(host.rt());
        let expected_tickter = host.get_ticketer();
        assert_eq!(ticketer, expected_tickter)
    }

    #[test]
    fn entry_native_deposit_succeeds() {
        let mut host = JstzMockHost::default();
        let deposit = MockNativeDeposit::default();
        host.add_internal_message(&deposit);
        host.rt().run_level(run);
        let tx = &mut Transaction::default();
        tx.begin();
        match deposit.receiver {
            L1Address::Implicit(L1PublicKeyHash::Ed25519(tz1)) => {
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
        host.rt().run_level(run);
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
        host.rt().run_level(run);
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
