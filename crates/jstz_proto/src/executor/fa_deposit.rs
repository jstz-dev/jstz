use bincode::{Decode, Encode};
use derive_more::{Display, Error, From};
use http::{header::CONTENT_TYPE, HeaderMap, Method, Uri};
use jstz_api::http::body::HttpBody;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::{michelson::ticket::TicketHash, prelude::debug_msg};
use utoipa::ToSchema;

use crate::{
    context::{new_account::Amount, new_account::NewAddress, ticket_table::TicketTable},
    executor::smart_function,
    operation::{external::FaDeposit, RunFunction},
    receipt::Receipt,
    Result,
};

const FA_DEPOSIT_GAS_LIMIT: usize = usize::MAX;

// TODO: https://linear.app/tezos/issue/JSTZ-36/use-cryptos-from-tezos-crypto
// Properly represent the null address
const NULL_ADDRESS: &str = "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
const DEPOSIT_URI: &str = "/-/deposit";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
pub struct FaDepositReceipt {
    pub receiver: NewAddress,
    pub ticket_balance: Amount,
    #[bincode(with_serde)]
    pub run_function: Option<crate::receipt::RunFunctionReceipt>,
}

#[derive(Display, Debug, Error, From)]
pub enum FaDepositError {
    InvalidHeaderValue,
    InvalidUri,
}

fn deposit_to_receiver(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    receiver: &NewAddress,
    ticket_hash: &TicketHash,
    amount: Amount,
) -> Result<FaDepositReceipt> {
    let final_balance = TicketTable::add(rt, tx, receiver, ticket_hash, amount)?;
    Ok(FaDepositReceipt {
        receiver: receiver.clone(),
        ticket_balance: final_balance,
        run_function: None,
    })
}

fn new_run_function(
    http_body: HttpBody,
    proxy_contract: &NewAddress,
) -> Result<RunFunction> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json; charset=utf-8"
            .parse()
            .map_err(|_| FaDepositError::InvalidHeaderValue)?,
    );
    Ok(RunFunction {
        uri: Uri::builder()
            .scheme("tezos")
            .authority(proxy_contract.to_string())
            .path_and_query(DEPOSIT_URI)
            .build()
            .map_err(|_| FaDepositError::InvalidUri)?,
        method: Method::POST,
        headers,
        body: http_body,
        gas_limit: FA_DEPOSIT_GAS_LIMIT,
    })
}

fn deposit_to_proxy_contract(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: &FaDeposit,
    proxy_contract: &NewAddress,
) -> Result<FaDepositReceipt> {
    let run = new_run_function(deposit.to_http_body(), proxy_contract)?;
    let source = PublicKeyHash::from_base58(NULL_ADDRESS)?;
    let result = smart_function::run::execute(
        rt,
        tx,
        &NewAddress::User(source),
        run,
        deposit.hash(),
    );
    match result {
        Ok(run_receipt) => {
            if run_receipt.status_code.is_success() {
                let final_balance = TicketTable::add(
                    rt,
                    tx,
                    proxy_contract,
                    &deposit.ticket_hash,
                    deposit.amount,
                )?;
                Ok(FaDepositReceipt {
                    receiver: proxy_contract.clone(),
                    ticket_balance: final_balance,
                    run_function: Some(run_receipt),
                })
            } else {
                let mut result = deposit_to_receiver(
                    rt,
                    tx,
                    &deposit.receiver,
                    &deposit.ticket_hash,
                    deposit.amount,
                )?;
                result.run_function = Some(run_receipt);
                Ok(result)
            }
        }
        Err(error) => {
            debug_msg!(
                rt,
                "Failed to execute proxy function when performing fa deposit: {error:?}\n"
            );
            let result = deposit_to_receiver(
                rt,
                tx,
                &deposit.receiver,
                &deposit.ticket_hash,
                deposit.amount,
            )?;
            Ok(result)
        }
    }
}

fn execute_inner(
    rt: &mut impl HostRuntime,
    tx: &mut Transaction,
    deposit: &FaDeposit,
) -> Result<FaDepositReceipt> {
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
        .expect("Unreachable: Failed to execute fa deposit!\n");
    let operation_hash = deposit.hash();
    Receipt::new(
        operation_hash,
        Ok(crate::receipt::ReceiptContent::FaDeposit(content)),
    )
}

#[cfg(test)]
mod test {

    use std::io::empty;

    use jstz_core::kv::Transaction;
    use jstz_crypto::smart_function_hash::SmartFunctionHash;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{
            new_account::NewAddress, new_account::ParsedCode, ticket_table::TicketTable,
        },
        executor::fa_deposit::{FaDeposit, FaDepositReceipt},
        receipt::{Receipt, ReceiptContent, ReceiptResult},
    };

    fn mock_fa_deposit(proxy: Option<SmartFunctionHash>) -> FaDeposit {
        FaDeposit {
            inbox_id: 34,
            amount: 42,
            receiver: NewAddress::User(jstz_mock::account2()),
            proxy_smart_function: proxy.map(NewAddress::SmartFunction),
            ticket_hash: jstz_mock::ticket_hash1(),
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

        match receipt.result {
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
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

        match receipt.result {
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
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
        let source = NewAddress::User(jstz_mock::account1());
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
            &host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();

        // TODO: use sf address
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        let proxy_sfh = match proxy.clone() {
            NewAddress::User(_) => panic!("proxy is not a user address"),
            NewAddress::SmartFunction(sfh) => sfh,
        };

        let fa_deposit = mock_fa_deposit(Some(proxy_sfh.clone()));
        let ticket_hash = fa_deposit.ticket_hash.clone();

        let Receipt { result: inner, .. } =
            super::execute(&mut host, &mut tx, fa_deposit);

        match inner {
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
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
        let source = NewAddress::User(jstz_mock::account1());
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
            &host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();

        // TODO: use sf address
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        let proxy_sfh = match proxy.clone() {
            NewAddress::User(_) => panic!("proxy is not a user address"),
            NewAddress::SmartFunction(sfh) => sfh,
        };

        let fa_deposit1 = mock_fa_deposit(Some(proxy_sfh.clone()));
        let ticket_hash = fa_deposit1.ticket_hash.clone();

        let _ = super::execute(&mut host, &mut tx, fa_deposit1);

        let fa_deposit2 = mock_fa_deposit(Some(proxy_sfh.clone()));

        let Receipt { result: inner, .. } =
            super::execute(&mut host, &mut tx, fa_deposit2);

        match inner {
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
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
        let source = NewAddress::User(jstz_mock::account1());
        let code = r#"
        export default (request) => {
            const url = new URL(request.url)
            return Response.error();
        }
        "#;
        let parsed_code = ParsedCode::try_from(code.to_string()).unwrap();
        let proxy = crate::executor::smart_function::Script::deploy(
            &host,
            &mut tx,
            &source,
            parsed_code,
            100,
        )
        .unwrap();

        // TODO: use sf address
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        let proxy = match proxy {
            NewAddress::User(_) => panic!("proxy is not a user address"),
            NewAddress::SmartFunction(sfh) => sfh,
        };

        let fa_deposit = mock_fa_deposit(Some(proxy.clone()));
        let expected_receiver = fa_deposit.receiver.clone();
        let ticket_hash = fa_deposit.ticket_hash.clone();

        let Receipt { result: inner, .. } =
            super::execute(&mut host, &mut tx, fa_deposit);

        match inner {
            ReceiptResult::Success(ReceiptContent::FaDeposit(FaDepositReceipt {
                receiver,
                ticket_balance,
                run_function,
            })) => {
                assert_eq!(500, run_function.unwrap().status_code);
                assert_eq!(expected_receiver, receiver);
                assert_eq!(42, ticket_balance);
                let proxy_balance = TicketTable::get_balance(
                    &mut host,
                    &mut tx,
                    &NewAddress::SmartFunction(proxy),
                    &ticket_hash,
                )
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
