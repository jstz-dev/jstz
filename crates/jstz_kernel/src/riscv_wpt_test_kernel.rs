#![cfg(feature = "riscv_wpt_test_kernel")]
use jstz_runtime::tests::wpt::run_wpt_tests;
use tezos_smart_rollup::{entrypoint, host::Runtime, prelude::debug_msg};

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    debug_msg!(rt, "Starting Jstz WPT test kernel\n");
    run_wpt_tests().await?;
    //jstz_kernel::riscv_kernel::run(rt);
}
