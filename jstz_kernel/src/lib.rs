use std::str;
use tezos_smart_rollup::{
    inbox::InboxMessage,
    kernel_entry,
    michelson::MichelsonUnit,
    prelude::{debug_msg, Runtime},
};

fn read_message(rt: &mut impl Runtime) -> Option<String> {
    let input = rt.read_input().ok()??;
    let _ = rt.mark_for_reboot();

    let (_, message) = InboxMessage::<MichelsonUnit>::parse(input.as_ref()).ok()?;
    debug_msg!(rt, "{message:?}\n");

    let InboxMessage::External(payload) = message else {
        return None
    };

    Some(String::from_utf8_lossy(payload).to_string())
}

fn handle_message(rt: &mut impl Runtime, msg: &str) {
    debug_msg!(rt, "Evaluating: {msg:?}\n");
    let res = jstz_core::evaluate_from_bytes(msg);
    debug_msg!(rt, "Result: {res:?}\n");
}

// kernel entry
pub fn entry(rt: &mut impl Runtime) {
    debug_msg!(rt, "Hello, kernel!\n");

    if let Some(msg) = read_message(rt) {
        handle_message(rt, &msg)
    }
}

kernel_entry!(entry);
