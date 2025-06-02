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
pub use jstz_runtime::{Kv, KvValue};
mod parsed_code;
pub use parsed_code::ParsedCode;
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
