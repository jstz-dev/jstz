use crate::{
    context::{account::Amount, ticket_table::TicketTable},
    executor::smart_function,
    operation::{external::FaDeposit, RunFunction},
    receipt::Receipt,
    Result,
};
use derive_more::{Display, Error, From};
use http::{header::CONTENT_TYPE, HeaderMap, Method, Uri};
use jstz_api::http::body::HttpBody;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Blake2b, public_key_hash::PublicKeyHash};
use serde::{Deserialize, Serialize};

const FA_DEPOSIT_GAS_LIMIT: usize = usize::MAX;

// TODO: https://linear.app/tezos/issue/JSTZ-36/use-cryptos-from-tezos-crypto
// Properly represent the null address
const NULL_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
const DEPOSIT_URI: &str = "/-/deposit";

#[derive(Display, Debug, Error, From)]
pub enum FaDepositError {
    ProxySmartFunctionError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaDepositReceiptContent {
    pub receiver: PublicKeyHash,
    pub ticket_balance: Amount,
    pub run_function: Option<crate::receipt::RunFunction>,
}

fn deposit_to_receiver(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    receiver: &PublicKeyHash,
    ticket_hash: &Blake2b,
    amount: Amount,
) -> Result<FaDepositReceiptContent> {
    let final_balance = TicketTable::add(rt, tx, receiver, ticket_hash, amount)?;
    Ok(FaDepositReceiptContent {
        receiver: receiver.clone(),
        ticket_balance: final_balance,
        run_function: None,
    })
}

fn new_run_function(http_body: HttpBody, proxy_contract: &PublicKeyHash) -> RunFunction {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json; charset=utf-8".parse().unwrap(),
    );
    RunFunction {
        uri: Uri::builder()
            .scheme("tezos")
            .authority(proxy_contract.to_string())
            .path_and_query(DEPOSIT_URI)
            .build()
            .unwrap(),
        method: Method::POST,
        headers,
        body: http_body,
        gas_limit: FA_DEPOSIT_GAS_LIMIT,
    }
}

fn deposit_to_proxy_contract(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: &FaDeposit,
    proxy_contract: &PublicKeyHash,
) -> Result<FaDepositReceiptContent> {
    let run = new_run_function(deposit.to_http_body(), proxy_contract);
    let source = PublicKeyHash::from_base58(NULL_ADDRESS).unwrap();
    let run_receipt = smart_function::run::execute(rt, tx, &source, run, deposit.hash())?;
    if run_receipt.status_code != 200 {
        let mut result = deposit_to_receiver(
            rt,
            tx,
            &deposit.receiver,
            &deposit.ticket_hash,
            deposit.amount,
        )?;
        result.run_function = Some(run_receipt);
        Ok(result)
    } else {
        let final_balance = TicketTable::add(
            rt,
            tx,
            proxy_contract,
            &deposit.ticket_hash,
            deposit.amount,
        )?;
        Ok(FaDepositReceiptContent {
            receiver: proxy_contract.clone(),
            ticket_balance: final_balance,
            run_function: Some(run_receipt),
        })
    }
}

fn execute_inner(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: &FaDeposit,
) -> Result<FaDepositReceiptContent> {
    match &deposit.proxy_smart_function {
        None => deposit_to_receiver(
            rt,
            tx,
            &deposit.receiver,
            &deposit.ticket_hash,
            deposit.amount,
        ),
        Some(proxy_contract) => {
            deposit_to_proxy_contract(rt, tx, deposit, proxy_contract)
        }
    }
}

pub fn execute(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: FaDeposit,
) -> Receipt {
    let content = execute_inner(rt, tx, &deposit)
        .expect("Unreachable: Failed to execute fa deposit!");
    let operation_hash = deposit.hash();
    Receipt::new(
        operation_hash,
        Ok(crate::receipt::Content::FaDeposit(content)),
    )
}

#[cfg(test)]
mod test {

    use std::io::empty;

    use jstz_core::kv::Transaction;
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_mock::mock;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{account::ParsedCode, ticket_table::TicketTable},
        executor::fa_deposit::{FaDeposit, FaDepositReceiptContent},
        receipt::{Content, Receipt},
    };

    fn mock_fa_deposit(proxy: Option<PublicKeyHash>) -> FaDeposit {
        FaDeposit {
            inbox_id: 34,
            amount: 42,
            receiver: mock::account2(),
            proxy_smart_function: proxy,
            ticket_hash: mock::ticket_hash1(),
        }
    }

    #[test]
    fn execute_fa_deposit_into_account_succeeds() {
        let fa_deposit = mock_fa_deposit(None);
        let expected_receiver = fa_deposit.receiver.clone();
        let ticket_hash = fa_deposit.ticket_hash.clone();
        let expected_balance = fa_deposit.amount;
        let expected_hash = fa_deposit.hash();
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();
        let receipt = super::execute(&mut host, &mut tx, fa_deposit);

        assert_eq!(expected_hash, *receipt.hash());

        match receipt.inner {
            Ok(Content::FaDeposit(FaDepositReceiptContent {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(expected_receiver, receiver);
                assert_eq!(expected_balance, ticket_balance);
                assert!(run_function.is_none());

                let balance = TicketTable::get_balance(
                    &mut host,
                    &mut tx,
                    &expected_receiver,
                    &ticket_hash,
                )
                .unwrap();
                assert_eq!(expected_balance, balance);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn execute_multiple_fa_deposit_into_account_succeeds() {
        let fa_deposit1 = mock_fa_deposit(None);
        let fa_deposit2 = mock_fa_deposit(None);
        let expected_receiver = fa_deposit2.receiver.clone();
        let ticket_hash = fa_deposit2.ticket_hash.clone();
        let expected_hash = fa_deposit2.hash();
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();

        let _ = super::execute(&mut host, &mut tx, fa_deposit1);
        let receipt = super::execute(&mut host, &mut tx, fa_deposit2);

        assert_eq!(expected_hash, *receipt.hash());

        match receipt.inner {
            Ok(Content::FaDeposit(FaDepositReceiptContent {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(84, ticket_balance);
                assert_eq!(expected_receiver, receiver);
                assert!(run_function.is_none());
                let balance = TicketTable::get_balance(
                    &mut host,
                    &mut tx,
                    &expected_receiver,
                    &ticket_hash,
                )
                .unwrap();
                assert_eq!(84, balance);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn execute_fa_deposit_into_proxy_succeeds() {
        let mut host = MockHost::default();
        host.set_debug_handler(empty());
        let mut tx = Transaction::default();
        let source = mock::account1();
        let code = r#"
        export default (request) => {
            const url = new URL(request.url)
            if (url.pathname === "/-/deposit") {
                return new Response();
            }
            return Response.error();
        }
        "#;
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let proxy = crate::executor::smart_function::Script::deploy(
            &mut host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();
        let fa_deposit = mock_fa_deposit(Some(proxy.clone()));
        let ticket_hash = fa_deposit.ticket_hash.clone();

        let Receipt { inner, .. } = super::execute(&mut host, &mut tx, fa_deposit);

        match inner {
            Ok(Content::FaDeposit(FaDepositReceiptContent {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(42, ticket_balance);
                assert_eq!(proxy, receiver);
                assert!(run_function.is_some());
                let balance =
                    TicketTable::get_balance(&mut host, &mut tx, &proxy, &ticket_hash)
                        .unwrap();
                assert_eq!(42, balance);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn execute_multiple_fa_deposit_into_proxy_succeeds() {
        let mut host = MockHost::default();
        host.set_debug_handler(empty());
        let mut tx = Transaction::default();
        let source = mock::account1();
        let code = r#"
        export default (request) => {
            const url = new URL(request.url)
            if (url.pathname === "/-/deposit") {
                return new Response();
            }
            return Response.error();
        }
        "#;
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        tx.begin();
        let proxy = crate::executor::smart_function::Script::deploy(
            &mut host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();
        let fa_deposit1 = mock_fa_deposit(Some(proxy.clone()));
        let ticket_hash = fa_deposit1.ticket_hash.clone();

        let _ = super::execute(&mut host, &mut tx, fa_deposit1);

        let fa_deposit2 = mock_fa_deposit(Some(proxy.clone()));

        let Receipt { inner, .. } = super::execute(&mut host, &mut tx, fa_deposit2);

        match inner {
            Ok(Content::FaDeposit(FaDepositReceiptContent {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(84, ticket_balance);
                assert_eq!(proxy, receiver);
                assert!(run_function.is_some());
                let balance =
                    TicketTable::get_balance(&mut host, &mut tx, &proxy, &ticket_hash)
                        .unwrap();
                assert_eq!(84, balance);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn execute_fa_deposit_fails_when_proxy_contract_fails() {
        let mut host = MockHost::default();
        host.set_debug_handler(empty());
        let mut tx = Transaction::default();
        tx.begin();
        let source = mock::account1();
        let code = r#"
        export default (request) => {
            const url = new URL(request.url)
            return Response.error();
        }
        "#;
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let proxy = crate::executor::smart_function::Script::deploy(
            &mut host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();

        let fa_deposit = mock_fa_deposit(Some(proxy.clone()));
        let expected_receiver = fa_deposit.receiver.clone();
        let ticket_hash = fa_deposit.ticket_hash.clone();

        let Receipt { inner, .. } = super::execute(&mut host, &mut tx, fa_deposit);

        match inner {
            Ok(Content::FaDeposit(FaDepositReceiptContent {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(500, run_function.unwrap().status_code);
                assert_eq!(expected_receiver, receiver);
                assert_eq!(42, ticket_balance);
                let proxy_balance =
                    TicketTable::get_balance(&mut host, &mut tx, &proxy, &ticket_hash)
                        .unwrap();
                assert_eq!(0, proxy_balance);

                let receiver_balance = TicketTable::get_balance(
                    &mut host,
                    &mut tx,
                    &expected_receiver,
                    &ticket_hash,
                )
                .unwrap();
                assert_eq!(42, receiver_balance);
            }
            _ => panic!("Expected success"),
        }
    }
}
