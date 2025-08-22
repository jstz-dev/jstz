#![cfg(feature = "wpt_test_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    jstz_riscv_wpt_test_kernel::run(rt);
}
