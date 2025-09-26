use crate::{ext::NotSupported, runtime::RuntimeContext};
use deno_core::*;
use jstz_core::log_record::LogLevel;
use tezos_smart_rollup::prelude::debug_msg;

#[cfg(feature = "kernel")]
// Struct just for type validation for content to be logged. Having refs here to avoid cloning.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefLogRecord<'a> {
    pub address: &'a jstz_crypto::smart_function_hash::SmartFunctionHash,
    pub request_id: &'a str,
    pub level: LogLevel,
    pub text: &'a str,
}

// Level Description
//  0    debug
//  1    log, info, dir, dirxml
//  2    warn
//  3    error
#[op2(fast)]
pub fn op_debug_msg(
    op_state: &mut OpState,
    #[string] msg: &str,
    level: u32,
) -> Result<(), NotSupported> {
    let proto = op_state.try_borrow_mut::<RuntimeContext>();
    match proto {
        Some(proto) => {
            #[cfg(not(feature = "kernel"))]
            debug_msg!(proto.host, "[{}] {}", code_to_log_level(level), msg);

            #[cfg(feature = "kernel")]
            {
                let body = serde_json::to_string(&RefLogRecord {
                    address: &proto.address,
                    request_id: &proto.request_id,
                    level: code_to_log_level(level),
                    text: msg,
                })
                .unwrap_or_default();
                debug_msg!(
                    proto.host,
                    "{}{}\n",
                    jstz_core::log_record::LOG_PREFIX,
                    body
                );
            }
            Ok(())
        }
        None => Err(NotSupported { name: "console" }),
    }
}

fn code_to_log_level(code: u32) -> LogLevel {
    // Note that this ordering is different from the LogLevel enum values.
    match code {
        0 => LogLevel::DEBUG,
        1 => LogLevel::INFO,
        2 => LogLevel::WARN,
        _ => LogLevel::ERROR,
    }
}

extension!(
    jstz_console,
    deps = [deno_console],
    ops = [op_debug_msg],
    esm_entry_point = "ext:jstz_console/console.js",
    esm = [dir "src/ext/jstz_console", "console.js"],
);

#[cfg(test)]
mod tests {
    use jstz_core::log_record::LogLevel;

    use deno_error::JsErrorClass;

    use crate::{init_test_setup, JstzRuntime, JstzRuntimeOptions};

    #[test]
    fn code_to_log_level() {
        assert_eq!(super::code_to_log_level(0), LogLevel::DEBUG);
        assert_eq!(super::code_to_log_level(1), LogLevel::INFO);
        assert_eq!(super::code_to_log_level(2), LogLevel::WARN);
        assert_eq!(super::code_to_log_level(3), LogLevel::ERROR);
        assert_eq!(super::code_to_log_level(4), LogLevel::ERROR);
    }

    #[test]
    fn console_log() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "log_request";
        };
        let code = r#"console.log("hello")"#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = "[JSTZ:SMART_FUNCTION:LOG] {\"address\":\"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton\",\"requestId\":\"log_request\",\"level\":\"INFO\",\"text\":\"hello\\n\"}\n";
        #[cfg(not(feature = "kernel"))]
        let expected = "[INFO] hello\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_info() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "info_request";
        };
        let code = r#"console.info("hello")"#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = "[JSTZ:SMART_FUNCTION:LOG] {\"address\":\"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton\",\"requestId\":\"info_request\",\"level\":\"INFO\",\"text\":\"hello\\n\"}\n";
        #[cfg(not(feature = "kernel"))]
        let expected = "[INFO] hello\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_warn() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "warn_request";
        };
        let code = r#"console.warn("hello")"#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = "[JSTZ:SMART_FUNCTION:LOG] {\"address\":\"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton\",\"requestId\":\"warn_request\",\"level\":\"WARN\",\"text\":\"hello\\n\"}\n";
        #[cfg(not(feature = "kernel"))]
        let expected = "[WARN] hello\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_error() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "error_request";
        };
        let code = r#"console.error("hello")"#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = "[JSTZ:SMART_FUNCTION:LOG] {\"address\":\"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton\",\"requestId\":\"error_request\",\"level\":\"ERROR\",\"text\":\"hello\\n\"}\n";
        #[cfg(not(feature = "kernel"))]
        let expected = "[ERROR] hello\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_debug() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "debug_request";
        };
        let code = r#"console.debug("hello")"#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = "[JSTZ:SMART_FUNCTION:LOG] {\"address\":\"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton\",\"requestId\":\"debug_request\",\"level\":\"DEBUG\",\"text\":\"hello\\n\"}\n";
        #[cfg(not(feature = "kernel"))]
        let expected = "[DEBUG] hello\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_js_types() {
        init_test_setup! {
            runtime = runtime;
            sink = sink;
            request_id = "js_types";
        };
        let code = r#"
            console.info(123)
            console.info(false)
            console.info({ message: "abc" })
        "#;
        runtime.execute(code).unwrap();

        #[cfg(feature = "kernel")]
        let expected = r#"[JSTZ:SMART_FUNCTION:LOG] {"address":"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton","requestId":"js_types","level":"INFO","text":"123\n"}
[JSTZ:SMART_FUNCTION:LOG] {"address":"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton","requestId":"js_types","level":"INFO","text":"false\n"}
[JSTZ:SMART_FUNCTION:LOG] {"address":"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton","requestId":"js_types","level":"INFO","text":"{ message: \"abc\" }\n"}
"#;
        #[cfg(not(feature = "kernel"))]
        let expected = "[INFO] 123\n[INFO] false\n[INFO] { message: \"abc\" }\n";
        assert_eq!(sink.to_string(), expected);
    }

    #[test]
    fn console_not_supported() {
        let mut runtime = JstzRuntime::new(JstzRuntimeOptions::default());
        let code = r#"console.info("hello")"#;
        let err = runtime.execute(code).unwrap_err();
        assert_eq!("NotSupported", err.get_class());
        assert_eq!(
            "NotSupported: console is not supported\n    at Console.console.Console.noColorStdout (ext:jstz_console/console.js:4:44)\n    at console.info (ext:deno_console/01_console.js:3167:20)\n    at jstz://run:1:9",
             err.get_message()
        );
    }
}
