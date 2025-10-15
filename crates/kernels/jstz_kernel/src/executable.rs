#![cfg(feature = "riscv_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

fn entry_function(rt: &mut impl Runtime) {
    debug_msg!(rt, "Starting Jstz kernel\n");
    jstz_kernel::riscv_kernel::run(rt);
}

// kernel entry
#[cfg(not(feature = "native_kernel"))]
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    entry_function(rt);
}

// kernel entry for native kernel
#[cfg(feature = "native_kernel")]
#[entrypoint::main]
#[entrypoint::runtime(static_inbox = "../../../jstz_tps_bench/inbox.json")]
pub fn entry(rt: &mut impl Runtime) {
    entry_function(rt);
}
