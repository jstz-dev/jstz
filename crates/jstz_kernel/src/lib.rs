use jstz_core::kv::{Storage, Transaction};
use jstz_proto::{context::account::Account, executor, Result};
use tezos_crypto_rs::hash::ContractKt1Hash;
use tezos_smart_rollup::{
    kernel_entry,
    prelude::{debug_msg, Runtime},
    storage::path::RefPath,
};

use crate::inbox::{read_message, Message};

pub mod inbox;

const TICKETER: RefPath = RefPath::assert_from(b"/ticketer");

fn read_ticketer(rt: &impl Runtime) -> Option<ContractKt1Hash> {
    Storage::get(rt, &TICKETER).ok()?
}

fn handle_message(hrt: &mut (impl Runtime + 'static), message: Message) -> Result<()> {
    let mut tx = Transaction::new();

    match message {
        Message::Internal(external_operation) => {
            executor::execute_external_operation(hrt, &mut tx, external_operation)?
        }
        Message::External(signed_operation) => {
            debug_msg!(hrt, "External operation: {signed_operation:?}\n");
            let receipt = executor::execute_operation(hrt, &mut tx, signed_operation);
            debug_msg!(hrt, "Receipt: {receipt:?}\n");
            receipt.write(hrt, &mut tx)?
        }
    }

    tx.commit::<Account>(hrt)?;
    Ok(())
}

// kernel entry
pub fn entry(rt: &mut (impl Runtime + 'static)) {
    let ticketer = read_ticketer(rt);

    if let Some(message) = read_message(rt, ticketer.as_ref()) {
        handle_message(rt, message)
            .unwrap_or_else(|err| debug_msg!(rt, "[🔴] {err:?}\n"));
    }
}

kernel_entry!(entry);
