use fetch::{
    error::FetchError, fetch_handler::process_and_dispatch_request,
    http::convert_header_map, http::Body,
};
use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    kv::Transaction,
};
use jstz_runtime::runtime::ExecutionTimeout;
use url::Url;

use crate::{
    context::account::Addressable,
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
};
pub mod fetch;
pub use jstz_core::log_record::{LogRecord, LOG_PREFIX};
pub use jstz_runtime::{Kv, KvValue};
mod parsed_code;
pub use parsed_code::ParsedCode;
mod ledger;
pub mod oracle;

pub async fn run_toplevel_fetch(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt, crate::Error> {
    #[allow(unused)]
    let (timeout, clock) = ExecutionTimeout::new(500, 8);
    #[cfg(not(feature = "native"))]
    return Ok(run(
        hrt,
        tx,
        timeout,
        source_address,
        run_operation,
        operation_hash,
    )
    .await?);

    #[cfg(feature = "native")]
    return {
        let (setup_ok, cancellation_token) = clock.start();
        if let Ok(true) = setup_ok.await {
            let r = run(
                hrt,
                tx,
                timeout,
                source_address,
                run_operation,
                operation_hash,
            )
            .await?;
            cancellation_token.cancel();
            Ok(r)
        } else {
            Err(crate::Error::TimeoutSetupFailed)
        }
    };
}

async fn run(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    timeout: ExecutionTimeout,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt, Error> {
    let url =
        Url::parse(run_operation.uri.to_string().as_str()).map_err(FetchError::from)?;
    let body = run_operation.body.map(Body::Vector);
    let response: http::Response<Option<Vec<u8>>> = process_and_dispatch_request(
        JsHostRuntime::new(hrt),
        tx.clone(),
        timeout,
        Some(operation_hash),
        source_address.clone().into(),
        run_operation.method.to_string().into(),
        url,
        convert_header_map(run_operation.headers),
        body,
    )
    .await
    .into();
    Ok(RunFunctionReceipt {
        body: response.body().clone(),
        status_code: response.status().clone(),
        headers: response.headers().clone(),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    FetchError(#[from] FetchError),
    #[error(transparent)]
    ParsedCodeError(#[from] parsed_code::ParseError),
}

#[cfg(test)]
pub mod test_utils {
    use jstz_core::{
        host::{HostRuntime, JsHostRuntime},
        kv::Transaction,
    };
    use jstz_crypto::{
        public_key_hash::PublicKeyHash, smart_function_hash::SmartFunctionHash,
    };
    use tezos_smart_rollup_mock::MockHost;

    use crate::{
        context::account::{Account, Addressable, Amount},
        runtime::ParsedCode,
    };

    // Deploy a vec of smart functions from the same creator, each
    // with `amount` XTZ. Returns a vec of hashes corresponding to
    // each sf deployed
    pub fn deploy_smart_functions<const N: usize>(
        scripts: [&str; N],
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        creator: &impl Addressable,
        amount: Amount,
    ) -> [SmartFunctionHash; N] {
        let mut hashes = vec![];
        for i in 0..N {
            // Safety
            // Script is valid
            let hash = Account::create_smart_function(hrt, tx, creator, amount, unsafe {
                ParsedCode::new_unchecked(scripts[i].to_string())
            })
            .unwrap();
            hashes.push(hash);
        }

        hashes.try_into().unwrap()
    }

    pub fn setup<'a, const N: usize>(
        host: &mut MockHost,
        scripts: [&'a str; N],
    ) -> (
        JsHostRuntime<'static>,
        Transaction,
        PublicKeyHash,
        [SmartFunctionHash; N],
    ) {
        let mut host = JsHostRuntime::new(host);
        let mut tx = jstz_core::kv::Transaction::default();
        tx.begin();
        let source_address = jstz_mock::account1();
        let hashes =
            deploy_smart_functions(scripts, &mut host, &mut tx, &source_address, 0);
        (host, tx, source_address, hashes)
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn timeout() {
        use http::{HeaderMap, Method, StatusCode, Uri};
        use jstz_core::kv::Transaction;
        use jstz_crypto::hash::Blake2b;

        use crate::{
            context::account::{Account, Address},
            operation::RunFunction,
            runtime::ParsedCode,
        };

        let from = Address::User(jstz_mock::account1());
        let mut host = tezos_smart_rollup_mock::MockHost::default();
        let mut tx = Transaction::default();

        let parsed_code = ParsedCode("export default () => { for (;;) {} };".to_string());
        tx.begin();
        let f0_addr =
            Account::create_smart_function(&mut host, &mut tx, &from, 0, parsed_code)
                .unwrap();
        let parsed_code = ParsedCode(format!(
            "export default async () => await fetch(\"jstz://{f0_addr}/\");"
        ));
        let f1_addr =
            Account::create_smart_function(&mut host, &mut tx, &from, 0, parsed_code)
                .unwrap();
        let parsed_code = ParsedCode(format!(
            "export default async () => await fetch(\"jstz://{f1_addr}/\");"
        ));
        let func_addr =
            Account::create_smart_function(&mut host, &mut tx, &from, 0, parsed_code)
                .unwrap();
        tx.commit(&mut host).unwrap();

        tx.begin();
        let receipt = super::run_toplevel_fetch(
            &mut host,
            &mut tx,
            &from,
            RunFunction {
                uri: Uri::try_from(&format!("jstz://{}/", func_addr.to_base58_check()))
                    .unwrap(),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: None,
                gas_limit: 10000,
            },
            Blake2b::from(b"op_hash".as_ref()),
        )
        .await
        .unwrap();
        assert_eq!(receipt.status_code, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            String::from_utf8(receipt.body.unwrap()).unwrap(),
            r#"{"class":"RuntimeError","message":"Uncaught Error: execution terminated"}"#
        );
    }
}
