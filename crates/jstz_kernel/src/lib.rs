use jstz_core::kv::Storage;
use jstz_crypto::{public_key::PublicKey, smart_function_hash::SmartFunctionHash};
use tezos_smart_rollup::{entrypoint, prelude::Runtime, storage::path::RefPath};

pub mod inbox;
pub mod parsing;

#[cfg(feature = "riscv_kernel")]
mod riscv_kernel;

#[cfg(not(feature = "riscv_kernel"))]
mod wasm_kernel;

pub const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
pub const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

pub(crate) fn read_ticketer(rt: &impl Runtime) -> Option<SmartFunctionHash> {
    Storage::get(rt, &TICKETER).ok()?
}

pub(crate) fn read_injector(rt: &impl Runtime) -> Option<PublicKey> {
    Storage::get(rt, &INJECTOR).ok()?
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    #[cfg(not(feature = "riscv_kernel"))]
    wasm_kernel::run(rt);

    #[cfg(feature = "riscv_kernel")]
    riscv_kernel::run(rt);
}
