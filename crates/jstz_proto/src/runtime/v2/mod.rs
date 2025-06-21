use fetch::{
    error::FetchError, fetch_handler::process_and_dispatch_request,
    http::convert_header_map, http::Body,
};
use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    kv::Transaction,
};
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
    Ok(run(hrt, tx, source_address, run_operation, operation_hash).await?)
}

async fn run(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
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
}
