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
    esm = [dir "src/runtime/console", "console.js"],
);

#[cfg(test)]
mod test {

    #[test]
    fn console_log() {}

    #[test]
    fn console_info() {}

    #[test]
    fn console_warn() {}
    #[test]
    fn console_error() {}

    #[test]
    fn console_debug() {}

    #[test]
    fn console_js_types() {}
}
