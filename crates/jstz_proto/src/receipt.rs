use http::{HeaderMap, StatusCode};
use jstz_api::http::body::HttpBody;
use serde::{Deserialize, Serialize};

use crate::{context::account::Address, operation::OperationHash, Result};

pub type ReceiptResult<T> = std::result::Result<T, String>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    hash: OperationHash,
    pub inner: ReceiptResult<Content>,
}

impl Receipt {
    pub fn new(hash: OperationHash, inner: Result<Content>) -> Self {
        let inner = inner.map_err(|e| e.to_string());
        Self { hash, inner }
    }

    pub fn hash(&self) -> &OperationHash {
        &self.hash
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployContract {
    pub contract_address: Address,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunContract {
    pub body: HttpBody,
    #[serde(with = "http_serde::status_code")]
    pub status_code: StatusCode,
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Content {
    DeployContract(DeployContract),
    RunContract(RunContract),
}
