use http::{HeaderMap, Method, Uri};
use jstz_api::http::body::HttpBody;
use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Blake2b, public_key::PublicKey, signature::Signature};
use serde::{Deserialize, Serialize};

use crate::{
    context::account::{Account, Address, Amount, Nonce, ParsedCode},
    Error, Result,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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
        let next_nonce = Account::nonce(rt, tx, &self.source)?;

        if self.nonce == *next_nonce {
            next_nonce.increment();
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
            Content::DeployFunction(DeployFunction {
                function_code,
                account_credit,
            }) => Blake2b::from(
                format!("{}{}{}{}", source, nonce, function_code, account_credit)
                    .as_bytes(),
            ),
            Content::RunFunction(RunFunction {
                uri,
                method,
                headers,
                body,
                ..
            }) => Blake2b::from(
                format!(
                    "{}{}{}{}{:?}{:?}",
                    source, nonce, uri, method, headers, body
                )
                .as_bytes(),
            ),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct DeployFunction {
    pub function_code: ParsedCode,
    pub account_credit: Amount,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct RunFunction {
    #[serde(with = "http_serde::uri")]
    pub uri: Uri,
    #[serde(with = "http_serde::method")]
    pub method: Method,
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
    pub body: HttpBody,
    pub gas_limit: usize,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Content {
    DeployFunction(DeployFunction),
    RunFunction(RunFunction),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedOperation {
    pub public_key: PublicKey,
    signature: Signature,
    inner: Operation,
}

impl SignedOperation {
    pub fn new(public_key: PublicKey, signature: Signature, inner: Operation) -> Self {
        Self {
            public_key,
            signature,
            inner,
        }
    }

    pub fn hash(&self) -> Blake2b {
        self.inner.hash()
    }

    pub fn verify(self) -> Result<Operation> {
        // FIXME: Adding signature verification kills to the rollup???!??!?!?!
        let hash = self.inner.hash();
        self.signature.verify(&self.public_key, hash.as_ref())?;

        Ok(self.inner)
    }
}

pub mod external {
    use tezos_smart_rollup::michelson::ticket::TicketHash;

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Deposit {
        pub amount: Amount,
        pub reciever: Address,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct FaDeposit {
        // Inbox message id is unique to each message and
        // suitable as a nonce
        pub inbox_id: u32,
        // Amount to deposit
        pub amount: Amount,
        // Final deposit receiver address
        pub receiver: Address,
        // Optional proxy contract
        pub proxy_smart_function: Option<Address>,
        // Ticket hash
        pub ticket_hash: TicketHash,
    }

    impl FaDeposit {
        fn json(&self) -> serde_json::Value {
            serde_json::json!({
                "receiver": self.receiver,
                "amount": self.amount,
                "ticketHash": self.ticket_hash.to_string(),
            })
        }

        pub fn to_http_body(&self) -> HttpBody {
            let body = self.json();
            Some(String::as_bytes(&body.to_string()).to_vec())
        }

        pub fn hash(&self) -> OperationHash {
            let seed = self.inbox_id.to_be_bytes();
            Blake2b::from(seed.as_slice())
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExternalOperation {
    Deposit(external::Deposit),
}
