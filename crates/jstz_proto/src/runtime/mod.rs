pub mod v1;
pub use v1::{Kv, KvValue, ParsedCode, ProtocolData, LOG_PREFIX};

#[cfg(not(feature = "v2_runtime"))]
pub use v1::{LogLevel, LogRecord};
#[cfg(feature = "v2_runtime")]
pub use v2::{LogLevel, LogRecord};

#[cfg(feature = "riscv")]
pub mod v2;

#[cfg(not(feature = "v2_runtime"))]
pub use v1::run_toplevel_fetch;

#[cfg(feature = "v2_runtime")]
pub use v2::run_toplevel_fetch;
