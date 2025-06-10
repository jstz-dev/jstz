use crate::{context::account::Addressable, operation::RunFunction};

use jstz_core::{
    host::HostRuntime,
    kv::{Storage, Transaction},
};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::Deserialize;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup_host::path::{OwnedPath, RefPath};

use crate::{
    error::Result,
    executor::{fa_withdraw::FaWithdraw, withdraw::Withdrawal},
    receipt::RunFunctionReceipt,
    Error,
};

pub const JSTZ_HOST: &str = "jstz";
pub const WITHDRAW_PATH: &str = "/withdraw";
pub const FA_WITHDRAW_PATH: &str = "/fa-withdraw";

fn validate_withdraw_request<'de, T>(run: &'de RunFunction) -> Result<T>
where
    T: Deserialize<'de>,
{
    let method = run
        .method
        .as_str()
        .parse::<http::Method>()
        .map_err(|_| Error::InvalidHttpRequestMethod)?;

    if method != http::Method::POST {
        return Err(Error::InvalidHttpRequestMethod);
    }

    if run.body.is_none() {
        return Err(Error::InvalidHttpRequestBody);
    }
    let withdrawal = serde_json::from_slice(run.body.as_ref().unwrap())
        .map_err(|_| Error::InvalidHttpRequestBody)?;
    Ok(withdrawal)
}

pub(crate) fn execute(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    ticketer: &ContractKt1Hash,
    source: &impl Addressable,
    run: RunFunction,
) -> Result<RunFunctionReceipt> {
    let uri = run.uri.clone();
    if uri.host() != Some(JSTZ_HOST) {
        return Err(Error::InvalidHost);
    }
    match uri.path() {
        WITHDRAW_PATH => {
            // TODO: https://linear.app/tezos/issue/JSTZ-77/check-gas-limit-when-performing-native-withdraws
            // Check gas limit

            let withdrawal = validate_withdraw_request::<Withdrawal>(&run)?;
            crate::executor::withdraw::execute_withdraw(
                hrt, tx, source, withdrawal, ticketer,
            )?;
            let receipt = RunFunctionReceipt {
                body: None,
                status_code: http::StatusCode::OK,
                headers: http::HeaderMap::new(),
            };
            Ok(receipt)
        }
        FA_WITHDRAW_PATH => {
            let fa_withdraw = validate_withdraw_request::<FaWithdraw>(&run)?;
            let fa_withdraw_receipt_content = fa_withdraw.execute(
                hrt, tx, source, 1000, // fake gas limit
            )?;
            let receipt = RunFunctionReceipt {
                body: fa_withdraw_receipt_content.to_http_body(),
                status_code: http::StatusCode::OK,
                headers: http::HeaderMap::new(),
            };
            Ok(receipt)
        }
        _ => Err(Error::UnsupportedPath),
    }
}

pub fn execute_without_ticketer(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source: &impl Addressable,
    run: RunFunction,
) -> Result<RunFunctionReceipt> {
    let ticketer_path = OwnedPath::from(&RefPath::assert_from(b"/ticketer"));
    let ticketer: SmartFunctionHash =
        Storage::get(hrt, &ticketer_path)?.expect("ticketer should be set");
    execute(hrt, tx, &ticketer, source, run)
}

#[cfg(test)]
mod test {
    use http::{header, HeaderMap, Method, Uri};
    use jstz_core::kv::Transaction;
    use jstz_mock::host::JstzMockHost;
    use serde_json::json;
    use tezos_crypto_rs::hash::ContractKt1Hash;
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::{
            account::{Account, Address},
            ticket_table::TicketTable,
        },
        executor::fa_withdraw::{FaWithdraw, RoutingInfo, TicketInfo},
        operation::RunFunction,
        Error,
    };

    use super::*;
    fn withdraw_request() -> RunFunction {
        RunFunction {
            uri: Uri::try_from("jstz://jstz/withdraw").unwrap(),
            method: Method::POST,
            headers: HeaderMap::from_iter([(
                header::CONTENT_TYPE,
                "application/json".try_into().unwrap(),
            )]),
            body: Some(
                json!({
                    "amount": 10,
                    "receiver": jstz_mock::account2().to_base58().to_string(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            ),
            gas_limit: 10,
        }
    }

    fn fa_withdraw_request() -> RunFunction {
        let ticket_info = TicketInfo {
            id: 1234,
            content: Some(b"random ticket content".to_vec()),
            ticketer: jstz_mock::kt1_account1().into(),
        };
        let routing_info = RoutingInfo {
            receiver: Address::User(jstz_mock::account2()),
            proxy_l1_contract: jstz_mock::kt1_account1().into(),
        };
        let fa_withdrawal = FaWithdraw {
            amount: 10,
            routing_info,
            ticket_info,
        };

        RunFunction {
            uri: Uri::try_from("jstz://jstz/fa-withdraw").unwrap(),
            method: Method::POST,
            headers: HeaderMap::from_iter([(
                header::CONTENT_TYPE,
                "application/json".try_into().unwrap(),
            )]),
            body: Some(json!(fa_withdrawal).to_string().as_bytes().to_vec()),
            gas_limit: 10,
        }
    }

    #[test]
    fn execute_fails_on_invalid_host() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            uri: Uri::try_from("jstz://example.com/withdraw").unwrap(),
            ..withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(super::Error::InvalidHost)));
    }

    #[test]
    fn execute_fails_on_unsupported_path() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            uri: Uri::try_from("jstz://jstz/blahblah").unwrap(),
            ..withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(super::Error::UnsupportedPath)));
    }

    #[test]
    fn execute_wthdraw_fails_on_invalid_request_method() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            method: Method::GET,
            ..withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(
            result,
            Err(super::Error::InvalidHttpRequestMethod)
        ));
    }

    #[test]
    fn execute_wthdraw_fails_on_invalid_request_body() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            body: Some(
                json!({
                    "amount": 10,
                    "not_receiver": jstz_mock::account2().to_base58()
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            ),
            ..withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));

        let req = RunFunction {
            body: None,
            ..withdraw_request()
        };
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));
    }

    #[test]
    fn execute_withdraw_succeeds() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());

        tx.begin();
        Account::add_balance(&host, &mut tx, &source, 10).unwrap();
        tx.commit(&mut host).unwrap();

        let req = withdraw_request();
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();

        execute(&mut host, &mut tx, &ticketer, &source, req)
            .expect("Withdraw should not fail");

        tx.begin();
        assert_eq!(0, Account::balance(&host, &mut tx, &source).unwrap());

        let level = host.run_level(|_| {});
        assert_eq!(1, host.outbox_at(level).len());
    }

    #[test]
    fn execute_without_ticketer_succeeds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let rt = host.rt();

        tx.begin();
        Account::add_balance(rt, &mut tx, &source, 10).unwrap();
        tx.commit(rt).unwrap();

        let req = withdraw_request();

        execute_without_ticketer(rt, &mut tx, &source, req)
            .expect("Withdraw should not fail");

        tx.begin();
        assert_eq!(0, Account::balance(rt, &mut tx, &source).unwrap());

        let level = rt.run_level(|_| {});
        assert_eq!(1, rt.outbox_at(level).len());
    }

    #[test]
    fn execute_fa_withdraw_fails_on_invalid_request_method() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            method: Method::GET,
            ..fa_withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(
            result,
            Err(super::Error::InvalidHttpRequestMethod)
        ));
    }

    #[test]
    fn execute_fa_withdraw_fails_on_invalid_request_body() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());
        let req = RunFunction {
            body: Some(
                json!({
                    "amount": 10,
                    "not_receiver": jstz_mock::account2().to_base58()
                })
                .to_string()
                .as_bytes()
                .to_vec(),
            ),
            ..fa_withdraw_request()
        };
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));

        let req = RunFunction {
            body: None,
            ..withdraw_request()
        };
        let result = execute(&mut host, &mut tx, &ticketer, &source, req);
        assert!(matches!(result, Err(Error::InvalidHttpRequestBody)));
    }

    #[test]
    fn execute_fa_withdraw_succeeds() {
        let mut host = MockHost::default();
        let mut tx = Transaction::default();
        let source = Address::User(jstz_mock::account1());

        let ticket = TicketInfo {
            id: 1234,
            content: Some(b"random ticket content".to_vec()),
            ticketer: jstz_mock::kt1_account1().into(),
        }
        .to_ticket(1)
        .unwrap();

        tx.begin();
        TicketTable::add(&mut host, &mut tx, &source, &ticket.hash, 10).unwrap();
        tx.commit(&mut host).unwrap();

        let req = fa_withdraw_request();
        let ticketer =
            ContractKt1Hash::from_base58_check(jstz_mock::host::NATIVE_TICKETER).unwrap();

        execute(&mut host, &mut tx, &ticketer, &source, req)
            .expect("Withdraw should not fail");

        tx.begin();
        assert_eq!(0, Account::balance(&host, &mut tx, &source).unwrap());

        let level = host.run_level(|_| {});
        assert_eq!(1, host.outbox_at(level).len());
    }
}
