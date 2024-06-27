use boa_engine::Context;
use jstz_api::js_log::{JsLog, LogData};
use jstz_core::runtime;
use tezos_smart_rollup::prelude::debug_msg;

pub(crate) struct PrettyLogger;

impl JsLog for PrettyLogger {
    fn log(&self, log_data: LogData, _context: &mut Context<'_>) {
        let LogData {
            level,
            text,
            groups_len,
        } = log_data;

        let indent = 2 * groups_len;
        let symbol = level.symbol();

        runtime::with_js_hrt(|hrt| {
            for line in text.lines() {
                debug_msg!(hrt, "[{symbol}] {:>indent$}{line}\n", "");
            }
        });
    }
}
