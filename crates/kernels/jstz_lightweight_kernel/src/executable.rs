#![cfg(feature = "lightweight-kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    jstz_lightweight_kernel::run(rt);
}
