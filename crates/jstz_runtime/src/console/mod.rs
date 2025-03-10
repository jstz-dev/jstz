use deno_core::*;
use tezos_smart_rollup::prelude::debug_msg;

use crate::JstzHostRuntime;

// Level Description
//  0    debug
//  1    log, info
//  2    warn
//  3    error
#[op2(fast)]
pub fn op_debug_msg(op_state: &mut OpState, #[string] msg: &str, level: u32) {
    let host = op_state.borrow_mut::<JstzHostRuntime>();
    debug_msg!(host, "{} {}", level_to_symbol(level), msg);
}

fn level_to_symbol(level: u32) -> &'static str {
    match level {
        0 => "D:",
        1 => "I:",
        2 => "W:",
        _ => "E:",
    }
}

extension!(
    jstz_console,
    deps = [deno_console],
    ops = [op_debug_msg],
    esm_entry_point = "ext:jstz_console/console.js",
    esm = [dir "src/console", "console.js"],
);

#[cfg(test)]
mod test {
    use crate::{test::MockHostRuntime, JstzRuntime};

    #[test]
    fn test_console_log() {
        let mut host = MockHostRuntime::init();
        let mut runtime = JstzRuntime::init(&mut (*host));
        let code = stringify!(
                console.log(123);
                console.log("log");
                console.info("info");
                console.warn("warn");
                console.error ("boom");
                console.debug("debug");
        );

        runtime
            .lazy_load_es_module_with_code("file://test", code)
            .unwrap();

        assert_eq!(
            String::from_utf8(host.sink().to_vec()).unwrap(),
            "I: \u{1b}[33m123\u{1b}[39m\nI: log\nI: info\nW: warn\nE: boom\nD: debug\n"
        );
    }
}
