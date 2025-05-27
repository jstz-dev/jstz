mod api;
mod fetch_handler;
mod host_script;
mod js_logger;
mod script;

pub use api::{Kv, KvValue, ProtocolApi, ProtocolData, WebApi};
use fetch_handler::{fetch, runtime_and_request_from_run_operation};
pub use js_logger::{LogRecord, LOG_PREFIX};
pub use script::ParsedCode;

use jstz_api::http::response::Response;
use jstz_core::{host::HostRuntime, kv::Transaction, runtime};
use tezos_smart_rollup::prelude::debug_msg;

use crate::{
    context::account::Addressable,
    error::Result,
    operation::{OperationHash, RunFunction},
    receipt::RunFunctionReceipt,
    Error,
};

pub async fn run_toplevel_fetch(
    hrt: &mut impl HostRuntime,
    tx: &mut Transaction,
    source_address: &(impl Addressable + 'static),
    run_operation: RunFunction,
    operation_hash: OperationHash,
) -> Result<RunFunctionReceipt> {
    let gas_limit = run_operation.gas_limit;
    let (mut rt, request) = runtime_and_request_from_run_operation(run_operation)?;

    let result = {
        let rt = &mut rt;
        runtime::enter_js_host_context(hrt, tx, || {
            jstz_core::future::block_on(async move {
                let result = fetch(source_address, operation_hash, &request, rt)?;
                rt.resolve_value(&result).await
            })
        })
    }
    .map_err(|err| {
        if rt.instructions_remaining() == 0 {
            Error::GasLimitExceeded
        } else {
            err.into()
        }
    })?;

    debug_msg!(hrt, "🚀 Smart function executed successfully with value: {:?} (in {:?} instructions)\n", result, gas_limit - rt.instructions_remaining());

    let response = Response::try_from_js(&result)?;
    let (http_parts, body) = Response::to_http_response(&response).into_parts();
    Ok(RunFunctionReceipt {
        body,
        status_code: http_parts.status,
        headers: http_parts.headers,
    })
}
