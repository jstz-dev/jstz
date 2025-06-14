#[cfg(not(feature = "v2_runtime"))]
pub mod v1;
#[cfg(not(feature = "v2_runtime"))]
pub use v1::{run_toplevel_fetch, Kv, KvValue, ParsedCode, LogRecord, LOG_PREFIX};

#[cfg(feature = "v2_runtime")]
pub mod v2;
#[cfg(feature = "v2_runtime")]
pub use v2::{run_toplevel_fetch, Kv, KvValue, ParsedCode, LogRecord, LOG_PREFIX};
