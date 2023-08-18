use jstz_core::JstzRuntime;
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

fn handle_message<H: Runtime + 'static>(rt: &mut H, msg: &str) {
    debug_msg!(rt, "Evaluating: {msg:?}\n");

    // Initialize runtime
    let mut jstz_runtime = JstzRuntime::new(rt);
    jstz_runtime.register_global_api::<jstz_api::ConsoleApi>();
    jstz_runtime.register_global_api::<jstz_api::LedgerApi>();

    // Eval
    let res = jstz_runtime.eval(msg);
    debug_msg!(rt, "Result: {res:?}\n");
}

// kernel entry
pub fn entry<H: Runtime + 'static>(rt: &mut H) {
    debug_msg!(rt, "Hello, kernel!\n");

    if let Some(msg) = read_message(rt) {
        handle_message(rt, &msg)
    }
}

kernel_entry!(entry);
