use crate::types::fa_deposit::FaDepositReceipt;
use crate::types::fa_withdraw::FaWithdrawReceipt;
use crate::types::operation::{Address, OperationHash};
use http::{HeaderMap, StatusCode};
use jstz_api::http::body::HttpBody;
use jstz_proto::receipt::{
    DeployFunctionReceipt as DeployFunctionReceiptInternal,
    DepositReceipt as DepositReceiptInternal, Receipt as ReceiptInternal,
    ReceiptContent as ReceiptContentInternal, ReceiptResult as ReceiptResultInternal,
    RunFunctionReceipt as RunFunctionReceiptInternal,
};
use jstz_utils::api_map_to;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[api_map_to(ReceiptResultInternal)]
#[derive(Debug, Clone, ToSchema, Serialize, Deserialize)]
#[serde(tag = "_type", content = "inner")]
pub enum ReceiptResult {
    #[schema(title = "Success")]
    Success(ReceiptContent),
    #[schema(title = "Failure")]
    Failed(String),
}

#[api_map_to(ReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Receipt {
    pub hash: OperationHash,
    pub result: ReceiptResult,
}

#[api_map_to(DeployFunctionReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeployFunctionReceipt {
    pub address: Address,
}

#[api_map_to(RunFunctionReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunFunctionReceipt {
    #[schema(schema_with = crate::types::operation::openapi::http_body_schema)]
    pub body: HttpBody,
    /// Valid status code
    #[serde(with = "http_serde::status_code")]
    #[schema(value_type = usize)]
    pub status_code: StatusCode,
    /// Any valid HTTP headers
    #[serde(with = "http_serde::header_map")]
    #[schema(schema_with = crate::types::operation::openapi::http_headers)]
    pub headers: HeaderMap,
}

#[api_map_to(DepositReceiptInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DepositReceipt {
    pub account: Address,
    pub updated_balance: u64,
}

#[api_map_to(ReceiptContentInternal)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "_type")]
pub enum ReceiptContent {
    #[schema(title = "DeployFunction")]
    DeployFunction(DeployFunctionReceipt),
    #[schema(title = "RunFunction")]
    RunFunction(RunFunctionReceipt),
    #[schema(title = "Deposit")]
    Deposit(DepositReceipt),
    #[schema(title = "FaDeposit")]
    FaDeposit(FaDepositReceipt),
    #[schema(title = "FaWithdraw")]
    FaWithdraw(FaWithdrawReceipt),
}
