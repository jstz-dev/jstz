#![cfg(feature = "wpt_test_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    debug_msg!(rt, "Starting Jstz RISC-V WPT test kernel\n");
    jstz_riscv_wpt_test_kernel::run(rt);
}
