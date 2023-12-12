use boa_engine::prelude::Context;
pub use jstz_api::JsLog;
use jstz_api::{LogData, LogLevel};
use jstz_core::{host::HostRuntime, host_defined, runtime::with_global_host};
use serde::Deserialize;
use serde::Serialize;
use tezos_smart_rollup::prelude::debug_msg;

use crate::api::TraceData;
use crate::context::account::Address;

pub const LOG_PREFIX: &str = "[JSTZ:SMART_FUNCTION:LOG] ";

#[derive(Serialize, Deserialize, Debug)]
pub struct LogRecord {
    pub contract_address: Address,
    pub request_id: String,
    pub level: LogLevel,
    pub text: String,
}

impl LogRecord {
    pub fn new(log_data: LogData, context: &mut Context<'_>) -> Self {
        host_defined!(context, host_defined);
        let trace_data = host_defined
            .get::<TraceData>()
            .expect("TraceData not found");

        let LogData {
            level,
            text,
            groups_len,
        } = log_data;

        let indent = 2 * groups_len;
        LogRecord {
            contract_address: trace_data.contract_address.clone(),
            request_id: trace_data.operation_hash.to_string(),
            level,
            text: " ".repeat(indent) + &text,
        }
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize JSONLog")
    }

    pub fn from_string(s: &str) -> Self {
        serde_json::from_str::<LogRecord>(s).expect("Failed to deserialize JSONLog")
    }
}

pub(super) struct JsonLogger;

impl JsLog for JsonLogger {
    fn log(&self, log_data: LogData, context: &mut Context<'_>) {
        let log_record = LogRecord::new(log_data, context).to_string();
        with_global_host(|rt| {
            rt.write_debug(&(LOG_PREFIX.to_string() + &log_record + "\n"));
        });
    }
    fn flush(&self) {
        panic!("JsonLogger does not support flush")
    }
}

pub struct PrettyLogger;

impl JsLog for PrettyLogger {
    fn log(&self, log_data: LogData, _context: &mut Context<'_>) {
        let LogData {
            level,
            text,
            groups_len,
        } = log_data;

        let indent = 2 * groups_len;
        let symbol = level.symbol();
        with_global_host(|rt| {
            for line in text.lines() {
                debug_msg!(rt, "[{symbol}] {:>indent$}{line}\n", "");
            }
        });
    }
    fn flush(&self) {
        panic!("PrettyLogger does not support flush")
    }
}
