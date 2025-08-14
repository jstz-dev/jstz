#[cfg(feature = "v2_runtime")]
use crate::runtime::v2::oracle::RequestId;
use crate::{
    context::account::Address,
    executor::{fa_deposit::FaDepositReceipt, fa_withdraw::FaWithdrawReceipt},
    operation::OperationHash,
    HttpBody, Result,
};
use bincode::{Decode, Encode};
use http::{HeaderMap, StatusCode};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, ToSchema, Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "_type", content = "inner")]
pub enum ReceiptResult {
    #[schema(title = "Success")]
    Success(ReceiptContent),
    #[schema(title = "Failure")]
    Failed(String),
}

impl From<Result<ReceiptContent>> for ReceiptResult {
    fn from(value: Result<ReceiptContent>) -> Self {
        match value {
            Ok(ok) => ReceiptResult::Success(ok),
            Err(err) => ReceiptResult::Failed(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
pub struct Receipt {
    #[bincode(with_serde)]
    hash: OperationHash,
    pub result: ReceiptResult,
}

impl Receipt {
    pub fn new(hash: OperationHash, inner: Result<ReceiptContent>) -> Self {
        Self {
            hash,
            result: inner.into(),
        }
    }

    pub fn hash(&self) -> &OperationHash {
        &self.hash
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
pub struct DeployFunctionReceipt {
    pub address: SmartFunctionHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunFunctionReceipt {
    pub body: HttpBody,
    /// Valid status code
    #[serde(with = "http_serde::status_code")]
    #[schema(value_type = usize)]
    pub status_code: StatusCode,
    /// Any valid HTTP headers
    #[serde(with = "http_serde::header_map")]
    #[schema(schema_with = crate::operation::openapi::response_headers)]
    pub headers: HeaderMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct DepositReceipt {
    pub account: Address,
    pub updated_balance: u64,
}

#[cfg(feature = "v2_runtime")]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct OracleResponseReceipt {
    pub request_id: RequestId,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode)]
#[serde(tag = "_type")]
pub enum ReceiptContent {
    #[schema(title = "DeployFunction")]
    DeployFunction(DeployFunctionReceipt),
    #[schema(title = "RunFunction")]
    RunFunction(#[bincode(with_serde)] RunFunctionReceipt),
    #[schema(title = "Deposit")]
    Deposit(DepositReceipt),
    #[schema(title = "FaDeposit")]
    FaDeposit(FaDepositReceipt),
    #[schema(title = "FaWithdraw")]
    FaWithdraw(FaWithdrawReceipt),
    #[cfg(feature = "v2_runtime")]
    #[schema(title = "OracleResponse")]
    OracleResponse(OracleResponseReceipt),
}
