use serde::Serialize;

use crate::{context::account::Address, operation::OperationHash, Result};

#[derive(Debug, Serialize)]
pub struct Receipt {
    hash: OperationHash,
    pub inner: Result<Content>,
}

impl Receipt {
    pub fn new(hash: OperationHash, inner: Result<Content>) -> Self {
        Self { hash, inner }
    }

    pub fn hash(&self) -> &OperationHash {
        &self.hash
    }
}

impl AsRef<Result<Content>> for Receipt {
    fn as_ref(&self) -> &Result<Content> {
        &self.inner
    }
}

#[derive(Debug, Serialize)]
pub struct DeployContract {
    pub contract_address: Address,
}

#[derive(Debug, Serialize)]
pub struct CallContract {
    pub result: String,
}

#[derive(Debug, Serialize)]
pub struct RunContract {
    pub result: String,
}

#[derive(Debug, Serialize)]
pub enum Content {
    DeployContract(DeployContract),
    CallContract(CallContract),
    RunContract(RunContract),
}
