use crate::runtime::ProtocolContext;
use deno_core::*;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use tezos_smart_rollup::prelude::debug_msg;
use utoipa::ToSchema;

#[cfg(feature = "kernel")]
mod kernel {
    pub(crate) const LOG_PREFIX: &str = "[JSTZ:SMART_FUNCTION:LOG]";

    // Struct just for type validation for content to be logged. Having refs here to avoid cloning.
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct RefLogRecord<'a> {
        pub address: &'a jstz_crypto::smart_function_hash::SmartFunctionHash,
        pub request_id: &'a str,
        pub level: super::LogLevel,
        pub text: &'a str,
    }
}

// Level Description
//  0    debug
//  1    log, info
//  2    warn
//  3    error
#[op2(fast)]
pub fn op_debug_msg(op_state: &mut OpState, #[string] msg: &str, level: u32) {
    let proto = op_state.borrow_mut::<ProtocolContext>();
    #[cfg(not(feature = "kernel"))]
    debug_msg!(proto.host, "[{}] {}", LogLevel::from(level), msg);

    #[cfg(feature = "kernel")]
    {
        let body = serde_json::to_string(&kernel::RefLogRecord {
            address: &proto.address,
            request_id: &proto.request_id,
            level: level.into(),
            text: msg,
        })
        .unwrap_or_default();
        debug_msg!(proto.host, "{} {}\n", kernel::LOG_PREFIX, body);
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub address: jstz_crypto::smart_function_hash::SmartFunctionHash,
    pub request_id: String,
    pub level: LogLevel,
    pub text: String,
}

impl LogRecord {
    #[allow(unused)]
    pub fn try_from_string(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Error = 3,
    Warn = 2,
    Info = 1,
    Debug = 0,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

impl TryFrom<&str> for LogLevel {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, <LogLevel as TryFrom<&str>>::Error> {
        match value {
            "ERROR" => Ok(LogLevel::Error),
            "WARN" => Ok(LogLevel::Warn),
            "INFO" => Ok(LogLevel::Info),
            "DEBUG" => Ok(LogLevel::Debug),
            _ => Err(format!("Invalid log level: {}", value)),
        }
    }
}

impl From<u32> for LogLevel {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Debug,
            1 => Self::Info,
            2 => Self::Warn,
            _ => Self::Error,
        }
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

    use crate::init_test_setup;

    mod log_record {
        use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};

        use crate::ext::jstz_console::{LogLevel, LogRecord};

        #[test]
        fn try_from_string() {
            assert!(LogRecord::try_from_string("").is_none());

            let r = LogRecord::try_from_string(r#"{"address":"KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton","requestId":"some_id","level":"INFO","text":"123\n"}"#).unwrap();
            assert_eq!(
                r.address,
                SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton")
                    .unwrap()
            );
            assert_eq!(r.level, LogLevel::Info);
            assert_eq!(r.request_id, "some_id");
            assert_eq!(r.text, "123\n");
        }
    }

    mod log_level {
        use crate::ext::jstz_console::LogLevel;

        #[test]
        fn fmt() {
            assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
            assert_eq!(LogLevel::Error.to_string(), "ERROR");
            assert_eq!(LogLevel::Info.to_string(), "INFO");
            assert_eq!(LogLevel::Warn.to_string(), "WARN");
        }

        #[test]
        fn try_from_str() {
            assert_eq!(LogLevel::try_from("DEBUG").unwrap(), LogLevel::Debug);
            assert_eq!(LogLevel::try_from("ERROR").unwrap(), LogLevel::Error);
            assert_eq!(LogLevel::try_from("INFO").unwrap(), LogLevel::Info);
            assert_eq!(LogLevel::try_from("WARN").unwrap(), LogLevel::Warn);
            assert_eq!(
                LogLevel::try_from("BLAH").unwrap_err().to_string(),
                "Invalid log level: BLAH"
            );
        }

        #[test]
        fn from_u32() {
            assert_eq!(LogLevel::from(0), LogLevel::Debug);
            assert_eq!(LogLevel::from(1), LogLevel::Info);
            assert_eq!(LogLevel::from(2), LogLevel::Warn);
            assert_eq!(LogLevel::from(3), LogLevel::Error);
            assert_eq!(LogLevel::from(4), LogLevel::Error);
        }
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
}
