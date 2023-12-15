use std::{any::Any, panic::RefUnwindSafe};

use jstz_core::kv::{Kv, Storage};
use jstz_proto::{executor, Result};
use serde::{Deserialize, Serialize};
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

unsafe fn handle_message(
    p_rt: *mut (impl Runtime + 'static),
    message: Message,
) -> Result<()> {
    let mut kv = Kv::new();
    let mut tx = kv.begin_transaction();
    let hrt = &mut *p_rt;

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

    kv.commit_transaction(hrt, tx)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PanicLogger {
    pub payload: Option<String>,
    pub message: Message,
}
impl PanicLogger {
    pub fn new(payload: &dyn Any, message: Message) -> Self {
        Self {
            payload: payload
                .downcast_ref::<String>()
                .map(String::clone)
                .or(payload.downcast_ref::<&str>().map(|s| s.to_string())),
            message,
        }
    }
}

// kernel entry
pub fn entry(rt: &mut (impl Runtime + RefUnwindSafe + 'static)) {
    let ticketer = read_ticketer(rt);
    if let Some(message) = read_message(rt, ticketer.as_ref()) {
        let p_rt = rt as *mut _;
        let handle = || {
            unsafe { handle_message(p_rt, message.clone()) }
                .unwrap_or_else(|err| debug_msg!(rt, "[🔴] {err:?}\n"));
        };
        //
        if let Err(err) = std::panic::catch_unwind(handle) {
            debug_msg!(
                rt,
                "[💥] Thread panicked! {:?}",
                PanicLogger::new(err.as_ref(), message)
            );
        }
    }
}

kernel_entry!(entry);
