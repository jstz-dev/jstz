#![cfg(feature = "riscv_kernel")]
use tezos_smart_rollup::{entrypoint, host::Runtime};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    jstz_kernel::riscv_kernel::run(rt);
}
