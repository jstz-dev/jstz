use jstz_crypto::hash::Hash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_kernel::inbox::{self, ParsedInboxMessage};
use tezos_smart_rollup::{
    entrypoint,
    prelude::{debug_msg, Runtime},
};

const DEFAULT_TICKETER_ADDRESS: &str = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";

/// A minimal kernel loop: reads inbox messages and logs them.
pub fn run(rt: &mut impl Runtime) {
    // Set up ticketer
    let ticketer = SmartFunctionHash::from_base58(DEFAULT_TICKETER_ADDRESS).unwrap();

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

        let deposit = MockNativeDeposit::default();
        host.add_internal_message(&deposit);

        host.rt().run_level(run);

        let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

        assert!(logged.contains(
        r#"[LW] Inbox message: Internal(Deposit(Deposit { inbox_id: InboxId { l1_level: 3760130, l1_message_id: 2 }, amount: 100,"#), "Logged output does not contain the expected message");

        assert!(
            logged.contains("[LW] Level info: End"),
            "Logged output does not contain the expected message"
        );
    }
}
