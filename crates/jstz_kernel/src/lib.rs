use inbox::Message;
use jstz_core::kv::{Storage, Transaction};
use jstz_crypto::{public_key::PublicKey, smart_function_hash::SmartFunctionHash};
use jstz_proto::executor;
use jstz_proto::Result;
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

pub mod inbox;
pub mod parsing;

#[cfg(feature = "riscv_kernel")]
pub mod riscv_kernel;

#[cfg(feature = "riscv_wpt_test_kernel")]
pub mod riscv_wpt_test_kernel;

#[cfg(not(any(feature = "riscv_kernel", feature = "riscv_wpt_test_kernel")))]
mod wasm_kernel;

pub const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");
pub const INJECTOR: RefPath = RefPath::assert_from(b"/injector");

pub(crate) fn read_ticketer(rt: &impl Runtime) -> SmartFunctionHash {
    Storage::get(rt, &TICKETER)
        .ok()
        .flatten()
        .expect("Ticketer not found")
}

pub(crate) fn read_injector(rt: &impl Runtime) -> PublicKey {
    Storage::get(rt, &INJECTOR)
        .ok()
        .flatten()
        .expect("Revealer not found")
}

pub async fn handle_message(
    hrt: &mut impl Runtime,
    message: Message,
    ticketer: &ContractKt1Hash,
    tx: &mut Transaction,
    injector: &PublicKey,
) -> Result<()> {
    match message {
        Message::Internal(internal_operation) => {
            let receipt =
                executor::execute_internal_operation(hrt, tx, internal_operation).await;
            receipt.write(hrt, tx)?
        }
        Message::External(signed_operation) => {
            debug_msg!(hrt, "External operation: {signed_operation:?}\n");
            let receipt = executor::execute_operation(
                hrt,
                tx,
                signed_operation,
                ticketer,
                injector,
            )
            .await;
            debug_msg!(hrt, "Receipt: {receipt:?}\n");
            receipt.write(hrt, tx)?
        }
    }
    Ok(())
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    #[cfg(not(any(feature = "riscv_kernel", feature = "riscv_wpt_test_kernel")))]
    wasm_kernel::run(rt);

    #[cfg(feature = "riscv_kernel")]
    riscv_kernel::run(rt);

    #[cfg(feature = "riscv_wpt_test_kernel")]
    riscv_wpt_test_kernel::entry(rt);
}
