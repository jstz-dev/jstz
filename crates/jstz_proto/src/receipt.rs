use crate::{
    context::account::Address,
    executor::{fa_deposit::FaDepositReceipt, fa_withdraw::FaWithdrawReceipt},
    operation::OperationHash,
    Result,
};
use http::{HeaderMap, StatusCode};
use jstz_api::http::body::HttpBody;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// pub type ReceiptResult<T> = std::result::Result<T, ReceiptError>;
#[derive(Debug, Clone, ToSchema, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum ReceiptResult {
    #[schema(title = "Success")]
    Success(ReceiptContent),
    #[schema(title = "Failure")]
    Failed { source: String },
}

impl From<Result<ReceiptContent>> for ReceiptResult {
    fn from(value: Result<ReceiptContent>) -> Self {
        match value {
            Ok(ok) => ReceiptResult::Success(ok),
            Err(err) => ReceiptResult::Failed {
                source: err.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Receipt {
    #[schema(value_type = String)]
    hash: OperationHash,
    pub inner: ReceiptResult,
}

impl Receipt {
    pub fn new(hash: OperationHash, inner: Result<ReceiptContent>) -> Self {
        Self {
            hash,
            inner: inner.into(),
        }
    }

    pub fn hash(&self) -> &OperationHash {
        &self.hash
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeployFunctionReceipt {
    pub address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunFunctionReceipt {
    #[schema(schema_with = crate::operation::openapi::http_body_schema)]
    pub body: HttpBody,
    /// Valid status code
    #[serde(with = "http_serde::status_code")]
    #[schema(value_type = usize)]
    pub status_code: StatusCode,
    /// Any valid HTTP headers
    #[serde(with = "http_serde::header_map")]
    #[schema(value_type = Object, additional_properties)]
    pub headers: HeaderMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DepositReceipt {
    pub account: Address,
    pub updated_balance: u64,
}

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
