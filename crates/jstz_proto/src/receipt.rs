use crate::{
    context::account::Address,
    executor::{fa_deposit::FaDepositReceipt, fa_withdraw::FaWithdrawReceipt},
    operation::OperationHash,
    Result,
};
use http::{HeaderMap, StatusCode};
use jstz_api::http::body::HttpBody;
use serde::{Deserialize, Serialize};

// pub type ReceiptResult<T> = std::result::Result<T, ReceiptError>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReceiptResult {
    Success(ReceiptContent),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub hash: OperationHash,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployFunctionReceipt {
    pub address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFunctionReceipt {
    pub body: HttpBody,
    /// Valid status code
    #[serde(with = "http_serde::status_code")]
    pub status_code: StatusCode,
    /// Any valid HTTP headers
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositReceipt {
    pub account: Address,
    pub updated_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReceiptContent {
    DeployFunction(DeployFunctionReceipt),
    RunFunction(RunFunctionReceipt),
    Deposit(DepositReceipt),
    FaDeposit(FaDepositReceipt),
    FaWithdraw(FaWithdrawReceipt),
}
