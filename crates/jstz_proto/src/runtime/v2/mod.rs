use std::{sync::OnceLock, time::Duration};

use crate::{
    context::account::Addressable,
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
};
use fetch::{
    error::FetchError,
    fetch_handler::process_and_dispatch_request,
    http::{convert_header_map, Body},
};
use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    kv::Transaction,
};
use jstz_runtime::runtime::Limiter;
use url::Url;
pub mod fetch;
pub use jstz_core::log_record::{LogRecord, LOG_PREFIX};
pub use jstz_runtime::{Kv, KvValue};
mod parsed_code;
pub use parsed_code::ParsedCode;
mod ledger;
pub mod oracle;
pub mod protocol_context;

#[cfg(all(
    not(any(target_arch = "riscv64", target_arch = "wasm32")),
    feature = "timeout"
))]
pub mod execution_tracker;
#[cfg(all(
    not(any(target_arch = "riscv64", target_arch = "wasm32")),
    feature = "timeout"
))]
pub use execution_tracker::*;

pub static SNAPSHOT: OnceLock<&'static [u8]> = OnceLock::new();

pub const TIMEOUT: u64 = 30;

pub async fn run_toplevel_fetch(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt, crate::Error> {
    return Ok(run(hrt, tx, source_address, run_operation, operation_hash).await?);
}

async fn run(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt, Error> {
    let RunFunction {
        uri,
        body,
        method,
        headers,
        gas_limit: _,
    } = run_operation;

    let url = Url::parse(uri.to_string().as_str()).map_err(FetchError::from)?;
    let body = body.0.map(Body::Vector);
    let hrt = JsHostRuntime::new(hrt);
    let source_address = source_address.clone().into();

    #[cfg(not(feature = "timeout"))]
    {
        let execution = async {
            let response: http::Response<Option<Vec<u8>>> = process_and_dispatch_request(
                hrt,
                tx.clone(),
                true,
                Some(operation_hash),
                source_address.clone(),
                source_address,
                method.to_string().into(),
                url,
                convert_header_map(headers),
                body,
                Limiter::default(),
            )
            .await
            .into();
            Ok(RunFunctionReceipt {
                body: response.body().clone().into(),
                status_code: response.status().clone(),
                headers: response.headers().clone(),
            })
        };

        return execution.await;
    }

    #[cfg(all(
        not(any(target_arch = "riscv64", target_arch = "wasm32")),
        feature = "timeout"
    ))]
    {
        use std::sync::Arc;

        let inner_tx = tx.inner();
        let exec_tracker = Arc::new(ExecutionTracker::default());
        let exec_tracker_clone = exec_tracker.clone();
        let execution_handle = tokio::task::spawn_blocking(move || {
            let execution = async {
                let response: http::Response<Option<Vec<u8>>> =
                    process_and_dispatch_request(
                        hrt,
                        inner_tx.into(),
                        true,
                        Some(operation_hash),
                        source_address.clone(),
                        source_address,
                        method.to_string().into(),
                        url,
                        convert_header_map(headers),
                        body,
                        Limiter::default(),
                        exec_tracker_clone,
                    )
                    .await
                    .into();
                Ok(RunFunctionReceipt {
                    body: response.body().clone().into(),
                    status_code: response.status().clone(),
                    headers: response.headers().clone(),
                })
            };
            let tokio_rt = tokio::runtime::Builder::new_current_thread().build();
            match tokio_rt {
                Ok(rt) => rt.block_on(execution),
                Err(err) => Err(TokioError::SpawnError(err)),
            }
        });

        match tokio::time::timeout(Duration::from_secs(TIMEOUT), execution_handle).await {
            Err(timeout_error) => {
                let mut executions = exec_tracker.executions.lock();
                while let Some((_, isolate_handle)) = executions.pop_last() {
                    while let false = isolate_handle.terminate_execution() {
                        tokio::time::sleep(Duration::from_millis(200)).await
                    }
                }
                let _ = tx.rollback();
                Err(TokioError::TimeoutError(timeout_error))?
            }
            Ok(spawn_result) => match spawn_result {
                Err(join_err) => Err(TokioError::JoinError(join_err))?,
                Ok(result) => Ok(result?),
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    FetchError(#[from] FetchError),
    #[error(transparent)]
    ParsedCodeError(#[from] parsed_code::ParseError),
    #[error(transparent)]
    OracleError(#[from] oracle::OracleError),

    #[cfg(all(
        not(any(target_arch = "riscv64", target_arch = "wasm32")),
        feature = "timeout"
    ))]
    #[error(transparent)]
    TokioError(#[from] TokioError),
}

#[cfg(all(
    not(any(target_arch = "riscv64", target_arch = "wasm32")),
    feature = "timeout"
))]
#[derive(Debug, thiserror::Error)]
pub enum TokioError {
    #[error("Join error: {0}")]
    JoinError(tokio::task::JoinError),
    #[error("Spawn error: {0}")]
    SpawnError(std::io::Error),
    #[error(
        "Smart function run timed out: Execution must complete within {TIMEOUT} seconds"
    )]
    TimeoutError(tokio::time::error::Elapsed),
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
