mod error;

pub mod context;
pub mod executor;
pub mod logger;
pub mod operation;
pub mod receipt;
pub mod storage;

use derive_more::{Deref, DerefMut};
pub use error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};
use utoipa::ToSchema;

pub mod runtime;

/// TODO: Move to appropriate component later
/// https://linear.app/tezos/issue/JSTZ-617/
pub type BlockLevel = u64;
pub type Gas = u64;

#[serde_as]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Deref,
    DerefMut,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[schema(
    title = "HTTP Body" , 
    value_type = Option<String>,
    description = "A HTTP body, which can be empty or contain data. Encoded as a base64 string."
)]
pub struct HttpBody(#[serde_as(as = "Option<Base64>")] pub Option<Vec<u8>>);

impl HttpBody {
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn unwrap(self) -> Vec<u8> {
        self.0.unwrap()
    }

    pub fn expect(self, msg: &str) -> Vec<u8> {
        self.0.expect(msg)
    }

    pub fn empty() -> Self {
        Self(None)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(Some(bytes))
    }

    pub fn from_string(s: String) -> Self {
        Self(Some(s.into_bytes()))
    }

    pub fn from_json(json: serde_json::Value) -> Self {
        Self(Some(serde_json::to_string(&json).unwrap().into_bytes()))
    }
}

impl From<HttpBody> for Option<Vec<u8>> {
    fn from(body: HttpBody) -> Self {
        body.0
    }
}

impl From<Option<Vec<u8>>> for HttpBody {
    fn from(bytes: Option<Vec<u8>>) -> Self {
        Self(bytes)
    }
}

impl From<String> for HttpBody {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<Vec<u8>> for HttpBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<serde_json::Value> for HttpBody {
    fn from(json: serde_json::Value) -> Self {
        Self::from_json(json)
    }
}

#[cfg(test)]
pub mod tests {
    use jstz_core::{host::HostRuntime, kv::Storage};
    use serde::de::DeserializeOwned;
    use tezos_smart_rollup::storage::path::OwnedPath;

    use crate::{operation::OperationHash, receipt::Receipt};
    pub use jstz_utils::test_util::DebugLogSink;

    // Helper to inpect fields in a receipt by tarversing the json path. Useful for debugging.
    // For example, to inpect the body of a successful RunFunctionReceipt, you can provide the path
    // vec!["result", "inner", "body"]. If you don't really care what the return type is and just
    // want to print field value, you can parameterize with `serde_json::Value`
    pub fn inspect_receipt<T: DeserializeOwned>(
        host: &impl HostRuntime,
        op_hash: OperationHash,
        path_into_receipt: Vec<&'static str>,
    ) -> T {
        let receipt_path = OwnedPath::try_from(format!("/jstz_receipt/{op_hash}"))
            .expect("Operation hash should exist");
        let receipt: Receipt = Storage::get(host, &receipt_path)
            .unwrap()
            .expect("Receipt should exist");
        let receipt = serde_json::to_value(&receipt).unwrap();
        let mut cursor = receipt.clone();
        for p in path_into_receipt {
            cursor = cursor[p].clone();
        }
        serde_json::from_value(cursor).unwrap()
    }
}
