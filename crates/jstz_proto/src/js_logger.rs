use std::fmt::Display;

use boa_engine::prelude::Context;
pub use jstz_api::js_log::{JsLog, LogData, LogLevel};
use jstz_core::{host::HostRuntime, host_defined, runtime};
use serde::Deserialize;
use serde::Serialize;

use crate::api::TraceData;
use crate::context::account::Address;

pub const LOG_PREFIX: &str = "[JSTZ:SMART_FUNCTION:LOG] ";

#[derive(Serialize, Deserialize)]
pub struct LogRecord {
    pub contract_address: Address,
    pub request_id: String,
    pub level: LogLevel,
    pub text: String,
}

impl Display for LogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &serde_json::to_string(self).expect("Failed to convert LogRecord to string"),
        )
    }
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

    pub fn try_from_string(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

pub(crate) struct JsonLogger;

impl JsLog for JsonLogger {
    fn log(&self, log_data: LogData, context: &mut Context<'_>) {
        let log_record = LogRecord::new(log_data, context).to_string();
        runtime::with_js_hrt(|hrt| {
            hrt.write_debug(&(LOG_PREFIX.to_string() + &log_record + "\n"));
        });
    }
}
