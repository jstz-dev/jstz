pub mod v1;
pub use v1::{LogRecord, ProtocolData, LOG_PREFIX};
#[cfg(feature = "riscv")]
pub mod v2;

#[cfg(not(feature = "v2_runtime"))]
pub use v1::{run_toplevel_fetch, Kv, KvValue, ParsedCode};

#[cfg(feature = "v2_runtime")]
pub use v2::{run_toplevel_fetch, Kv, KvValue, ParsedCode};
