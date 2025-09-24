use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
};

use jstz_core::kv::Storage;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_kernel::inbox::{self, ParsedInboxMessage};
use tezos_crypto_rs::hash::ContractKt1Hash;

/// A minimal kernel loop: reads inbox messages and logs them.
pub fn run(rt: &mut impl Runtime) {
    // Use the configured ticketer stored by the environment (same path as main kernel)
    let ticketer: ContractKt1Hash =
        Storage::get::<SmartFunctionHash>(rt, &jstz_kernel::TICKETER)
            .ok()
            .flatten()
            .expect("Ticketer not found")
            .into();

    // Drain messages this level and log them
    while let Some(parsed) = inbox::read_message(rt, &ticketer) {
        match parsed.content {
            ParsedInboxMessage::JstzMessage(msg) => {
                debug_msg!(rt, "[LW] Inbox message: {:?}\n", msg);
            }
            ParsedInboxMessage::LevelInfo(info) => {
                debug_msg!(rt, "[LW] Level info: {:?}\n", info);
            }
        }
    }
}

// kernel entry
#[entrypoint::main]
pub fn entry(rt: &mut impl Runtime) {
    run(rt);
}

#[cfg(test)]
mod tests {
    use super::run;
    use jstz_mock::{host::JstzMockHost, message::native_deposit::MockNativeDeposit};
    use std::sync::{Arc, Mutex};
    use tezos_smart_rollup_mock::DebugSink;

    #[derive(Clone, Default)]
    struct BufSink(Arc<Mutex<Vec<u8>>>);
    impl DebugSink for BufSink {
        fn write_all(&mut self, buffer: &[u8]) -> std::io::Result<()> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(())
        }
    }

    #[test]
    fn logs_internal_deposit_message() {
        let sink = BufSink::default();
        let buf = sink.0.clone();

        let mut host = JstzMockHost::new(true);
        host.set_debug_handler(sink);

        // Add a native deposit internal message
        let deposit = MockNativeDeposit::default();
        host.add_internal_message(&deposit);

        host.rt().run_level(run);

        let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(logged.contains("[LW] Inbox message:"));
    }
}
