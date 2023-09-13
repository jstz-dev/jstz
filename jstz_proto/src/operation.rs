use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Blake2b, public_key::PublicKey, signature::Signature};
use serde::{Deserialize, Serialize};

use crate::{
    context::account::{Account, Address, Amount, Nonce},
    Error, Result,
};

#[derive(Serialize, Deserialize)]
pub struct Operation {
    pub source: Address,
    pub nonce: Nonce,
    pub content: Content,
}

pub type OperationHash = Blake2b;

impl Operation {
    /// Returns the source of the operation
    pub fn source(&self) -> &Address {
        &self.source
    }

    /// Returns the nonce of the operation
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    /// Verify the nonce of the operation
    /// Returns the operation's
    pub fn verify_nonce(
        &self,
        rt: &impl HostRuntime,
        tx: &mut Transaction,
    ) -> Result<()> {
        let next_nonce = Account::nonce(rt, tx, &self.source)?.next();

        if self.nonce == next_nonce {
            Ok(())
        } else {
            Err(Error::InvalidNonce)
        }
    }

    /// Computes the operation hash.
    /// This is the hash which the client should sign
    pub fn hash(&self) -> OperationHash {
        let Operation {
            source,
            nonce,
            content,
        } = self;
        match content {
            Content::DeployContract(DeployContract {
                contract_code,
                contract_credit,
            }) => Blake2b::from(
                format!(
                    "{}{}{}{}",
                    source.to_string(),
                    nonce.to_string(),
                    contract_code,
                    contract_credit
                )
                .as_bytes(),
            ),
            Content::CallContract(CallContract { contract_address }) => Blake2b::from(
                format!(
                    "{}{}{}",
                    source.to_string(),
                    nonce.to_string(),
                    contract_address
                )
                .as_bytes(),
            ),
            Content::RunContract(RunContract {
                contract_address,
                contract_code,
            }) => Blake2b::from(
                format!(
                    "{}{}{}{}",
                    source.to_string(),
                    nonce.to_string(),
                    contract_address,
                    contract_code
                )
                .as_bytes(),
            ),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeployContract {
    pub contract_code: String,
    pub contract_credit: Amount,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CallContract {
    pub contract_address: Address,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RunContract {
    pub contract_address: Address,
    pub contract_code: String,
}

#[derive(Serialize, Deserialize)]
pub enum Content {
    DeployContract(DeployContract),
    CallContract(CallContract),
    RunContract(RunContract),
}

#[derive(Serialize, Deserialize)]
pub struct SignedOperation {
    public_key: PublicKey,
    signature: Signature,
    inner: Operation,
}

impl SignedOperation {
    pub fn hash(&self) -> Blake2b {
        self.inner.hash()
    }

    pub fn verify(self) -> Result<Operation> {
        let hash = self.inner.hash();
        self.signature.verify(&self.public_key, hash.as_ref())?;

        Ok(self.inner)
    }
}

pub mod external {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Deposit {
        pub amount: Amount,
        pub reciever: Address,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ContractOrigination {
        pub originating_address: Address,
        pub initial_balance: Amount,
        pub contract_code: String,
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExternalOperation {
    Deposit(external::Deposit),
    ContractOrigination(external::ContractOrigination),
}
