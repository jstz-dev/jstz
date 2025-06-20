use deno_core::{extension, op2, OpState};

use jstz_crypto::hash::Hash;
use jstz_runtime::ProtocolContext;

use crate::context::account::{Account, Address};

#[op2]
#[string]
fn op_self_address(state: &mut OpState) -> String {
    let proto = state.borrow_mut::<ProtocolContext>();
    proto.address.to_base58()
}

#[op2(fast)]
#[number]
fn op_balance(state: &mut OpState, #[string] address: String) -> Result<u64> {
    let ProtocolContext { host, tx, .. } = state.borrow_mut::<ProtocolContext>();
    let address = Address::from_base58(&address)?;
    Ok(Account::balance(host, tx, &address)?)
}

#[op2(fast)]
fn op_transfer(
    state: &mut OpState,
    #[string] dest_address: String,
    #[number] amount: u64,
) -> Result<()> {
    let ProtocolContext {
        host, tx, address, ..
    } = state.borrow_mut::<ProtocolContext>();
    let dest = Address::from_base58(&dest_address)?;
    Ok(Account::transfer(host, tx, address, &dest, amount)?)
}

pub type Result<T> = std::result::Result<T, LedgerError>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LedgerError {
    /// Errors resulting from the error enums of proto/core. They currently
    /// reference boa which is difficult to tease apart and has some error structs
    /// which are not thread safe
    ///
    /// Should be cleaned up as part of https://linear.app/tezos/issue/JSTZ-672
    #[class(generic)]
    #[error("{0}")]
    V1Error(String),
}

impl From<crate::error::Error> for LedgerError {
    fn from(value: crate::error::Error) -> Self {
        Self::V1Error(value.to_string())
    }
}

extension!(
    jstz_ledger,
    ops = [op_self_address, op_balance, op_transfer],
    esm_entry_point = "ext:jstz_ledger/ledger.js",
    esm = [dir "src/runtime/v2/ledger", "ledger.js"]
);

#[cfg(test)]
mod test {
    use jstz_core::host::JsHostRuntime;
    use jstz_utils::test_util::TOKIO_MULTI_THREAD;
    use url::Url;

    use crate::{
        context::account::Account,
        runtime::v2::{
            fetch::fetch_handler::process_and_dispatch_request, test_utils::*,
        },
    };

    #[test]
    fn self_address() {
        TOKIO_MULTI_THREAD.block_on(async {
            // Code
            let run = r#"export default async () => new Response(Ledger.selfAddress)"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, tx, source_address, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                None,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                run_address.to_string(),
                String::from_utf8(response.body.to_vec()).unwrap()
            )
        })
    }

    #[test]
    fn balance() {
        TOKIO_MULTI_THREAD.block_on(async {
            // Code
            let run = r#"export default async (request) => {
                let referrer = request.headers.get("referrer");
                let balance = Ledger.balance(referrer);
                return new Response(balance)
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (host, mut tx, source_address, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();
            Account::add_balance(&host, &mut tx, &source_address, 1_000_000_000).unwrap();

            // Run
            let response = process_and_dispatch_request(
                host,
                tx,
                None,
                source_address.into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                1_000_000_000,
                String::from_utf8(response.body.to_vec())
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
            )
        })
    }

    #[test]
    fn transfer() {
        TOKIO_MULTI_THREAD.block_on(async {
            // Code
            let run = r#"export default async (request) => {
                let referrer = request.headers.get("referrer");
                Ledger.transfer(referrer, 500 * 1000000);
                return new Response()
            }"#;

            // Setup
            let mut host = tezos_smart_rollup_mock::MockHost::default();
            let (mut host, mut tx, source_address, hashes) = setup(&mut host, [run]);
            let run_address = hashes[0].clone();
            Account::add_balance(&host, &mut tx, &run_address, 1_000_000_000).unwrap();

            // Run
            let _ = process_and_dispatch_request(
                JsHostRuntime::new(&mut host),
                tx.clone(),
                None,
                source_address.clone().into(),
                "GET".into(),
                Url::parse(format!("jstz://{}", run_address).as_str()).unwrap(),
                vec![],
                None,
            )
            .await;

            // Assert
            assert_eq!(
                500_000_000,
                Account::balance(&host, &mut tx, &run_address).unwrap()
            );

            assert_eq!(
                500_000_000,
                Account::balance(&host, &mut tx, &source_address).unwrap()
            )
        })
    }
}
