#![cfg(feature = "riscv_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    debug_msg!(rt, "Starting Jstz kernel\n");
    jstz_kernel::riscv_kernel::run(rt);
}
