use http::{HeaderMap, StatusCode};
use jstz_api::http::body::HttpBody;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    context::account::Address,
    executor::{fa_deposit::FaDepositReceipt, fa_withdraw::FaWithdrawReceipt},
    operation::OperationHash,
    Result,
};

pub type ReceiptResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Receipt {
    #[schema(value_type = String)]
    hash: OperationHash,
    #[schema(value_type = openapi::ReceiptResult<ReceiptContent>)]
    pub inner: ReceiptResult<ReceiptContent>,
}

impl Receipt {
    pub fn new(hash: OperationHash, inner: Result<ReceiptContent>) -> Self {
        let inner = inner.map_err(|e| e.to_string());
        Self { hash, inner }
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
pub enum ReceiptContent {
    DeployFunction(DeployFunctionReceipt),
    RunFunction(RunFunctionReceipt),
    Deposit,
    FaDeposit(FaDepositReceipt),
    FaWithdraw(FaWithdrawReceipt),
}

mod openapi {
    use utoipa::ToSchema;

    #[allow(dead_code)]
    #[derive(ToSchema)]
    pub enum ReceiptResult<T: ToSchema> {
        Ok(T),
        Err(String),
    }
}
