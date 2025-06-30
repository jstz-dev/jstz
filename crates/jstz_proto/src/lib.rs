mod error;

pub mod context;
#[cfg(feature = "v2_runtime")]
pub mod event;
pub mod executor;
pub mod logger;
pub mod operation;
pub mod receipt;
pub mod storage;

pub use error::{Error, Result};

pub mod runtime;

/// TODO: Move to appropriate component later
/// https://linear.app/tezos/issue/JSTZ-617/
pub type BlockLevel = u64;
pub type Gas = u64;
pub type HttpBody = Option<Vec<u8>>;

#[cfg(test)]
pub mod tests {
    use std::sync::{Arc, Mutex};

    use jstz_core::{host::HostRuntime, kv::Storage};
    use serde::de::DeserializeOwned;
    use tezos_smart_rollup::storage::path::OwnedPath;
    use tezos_smart_rollup_mock::DebugSink;

    use crate::{operation::OperationHash, receipt::Receipt};

    #[derive(Clone, Default)]
    pub struct DebugLogSink {
        pub inner: Arc<Mutex<Vec<u8>>>,
    }

    impl DebugSink for DebugLogSink {
        fn write_all(&mut self, buffer: &[u8]) -> std::io::Result<()> {
            self.inner.lock().unwrap().extend_from_slice(buffer);
            Ok(())
        }
    }

    impl DebugLogSink {
        pub fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(vec![])),
            }
        }

        pub fn content(&self) -> Arc<Mutex<Vec<u8>>> {
            self.inner.clone()
        }

        #[cfg(feature = "v2_runtime")]
        pub fn str_content(&self) -> String {
            let buf = self.inner.lock().unwrap();
            String::from_utf8(buf.to_vec()).unwrap()
        }

        #[cfg(feature = "v2_runtime")]
        pub fn lines(&self) -> Vec<String> {
            let str_content = self.str_content();
            str_content.split("\n").map(|s| s.to_string()).collect()
        }
    }

    // Helper to inpect fields in a receipt by tarversing the json path. Useful for debugging.
    // For example, to inpect the body of a successful RunFunctionReceipt, you can provide the path
    // vec!["result", "inner", "body"]. If you don't really care what the return type is and just
    // want to print field value, you can parameterize with `serde_json::Value`
    #[allow(unused)]
    fn inspect_receipt<T: DeserializeOwned>(
        host: &impl HostRuntime,
        op_hash: OperationHash,
        path_into_receipt: Vec<String>,
    ) -> T {
        let receipt_path =
            OwnedPath::try_from(format!("/jstz_receipt/{}", op_hash)).unwrap();
        let receipt: Receipt = Storage::get(host, &receipt_path).unwrap().unwrap();
        let receipt = serde_json::to_value(&receipt).unwrap();
        let mut cursor = receipt.clone();
        for p in path_into_receipt {
            cursor = cursor[p].clone();
        }
        serde_json::from_value(cursor).unwrap()
    }
}
