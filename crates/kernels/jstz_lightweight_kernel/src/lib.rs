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

                let bytes = rt.store_read_all(
                    &tezos_smart_rollup::storage::path::RefPath::assert_from(b"/key"),
                );
                let new_value = match bytes {
                    Ok(v) => {
                        let value = String::from_utf8_lossy(&v);
                        debug_msg!(rt, "[TEST] stored value: {:?}\n", value);
                        value.to_string() + "a"
                    }
                    Err(e) => {
                        debug_msg!(rt, "[TEST] error reading value: {:?}\n", e);
                        "a".to_string()
                    }
                };

                if let Err(e) = rt.store_write_all(
                    &tezos_smart_rollup::storage::path::RefPath::assert_from(b"/key"),
                    new_value.as_bytes(),
                ) {
                    debug_msg!(rt, "[TEST] error writing value: {:?}\n", e);
                }
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
    use jstz_utils::test_util::DebugLogSink;

    #[test]
    fn logs_internal_deposit_message() {
        let sink = DebugLogSink::new();
        let buf = sink.content();

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
