use crate::runtime::Protocol;
use deno_core::*;
use tezos_smart_rollup::prelude::debug_msg;

// Level Description
//  0    debug
//  1    log, info
//  2    warn
//  3    error
#[op2(fast)]
pub fn op_debug_msg(op_state: &mut OpState, #[string] msg: &str, level: u32) {
    let proto = op_state.borrow_mut::<Protocol>();
    debug_msg!(proto.host, "{} {}", level_to_symbol(level), msg);
}

fn level_to_symbol(level: u32) -> &'static str {
    match level {
        0 => "[DEBUG]",
        1 => "[INFO]",
        2 => "[WARN]",
        _ => "[ERROR]",
    }
}

extension!(
    jstz_console,
    deps = [deno_console],
    ops = [op_debug_msg],
    esm_entry_point = "ext:jstz_console/console.js",
    esm = [dir "src/jstz_console", "console.js"],
);

#[cfg(test)]
mod test {

    use crate::init_test_setup;

    #[test]
    fn console_log() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"console.log("hello")"#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[INFO] hello\n");
    }

    #[test]
    fn console_info() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"console.info("hello")"#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[INFO] hello\n");
    }

    #[test]
    fn console_warn() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"console.warn("hello")"#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[WARN] hello\n");
    }

    #[test]
    fn console_error() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"console.error("hello")"#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[ERROR] hello\n");
    }

    #[test]
    fn console_debug() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"console.debug("hello")"#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[DEBUG] hello\n");
    }

    #[test]
    fn console_js_types() {
        init_test_setup!(runtime, host, tx, sink, address);
        let code = r#"
            console.info(123)
            console.info(false)
            console.info({ message: "abc" })
        "#;
        runtime.execute(code).unwrap();
        assert_eq!(sink.to_string(), "[INFO] \u{1b}[33m123\u{1b}[39m\n[INFO] \u{1b}[33mfalse\u{1b}[39m\n[INFO] { message: \u{1b}[32m\"abc\"\u{1b}[39m }\n");
    }
}
