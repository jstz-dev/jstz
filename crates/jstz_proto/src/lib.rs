mod error;

pub mod context;
#[cfg(feature = "riscv")]
pub mod event;
pub mod executor;
pub mod operation;
pub mod receipt;
pub mod request_logger;
pub use error::{Error, Result};

pub mod runtime;

/// TODO: Move to appropriate component later
/// https://linear.app/tezos/issue/JSTZ-617/
pub type BlockLevel = u64;
pub type Gas = u64;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tezos_smart_rollup_mock::DebugSink;

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
    }
}
