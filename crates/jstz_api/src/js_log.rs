use boa_engine::{Context, JsNativeError, JsResult};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::str::FromStr;

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    ERROR = 1,
    WARN = 2,
    INFO = 3,
    LOG = 4,
}

pub const DEFAULT_LOG_LOG_LEVEL: LogLevel = LogLevel::LOG;

impl FromStr for LogLevel {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ERROR" => Ok(LogLevel::ERROR),
            "WARN" => Ok(LogLevel::WARN),
            "INFO" => Ok(LogLevel::INFO),
            "LOG" => Ok(LogLevel::LOG),
            _ => Err("no match"),
        }
    }
}

impl LogLevel {
    pub fn symbol(&self) -> char {
        match self {
            LogLevel::ERROR => '🔴',
            LogLevel::WARN => '🟠',
            LogLevel::INFO => '🟢',
            LogLevel::LOG => '🪵',
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct LogData {
    pub level: LogLevel,
    pub text: String,
    pub groups_len: usize,
}

// The implementor of this trait controls how console.log/warn/error etc is handled.
pub trait JsLog {
    fn log(&self, log_data: LogData, context: &mut Context<'_>);
    fn flush(&self) {}
}

thread_local! {
    /// Thread-local logger
    static CONSOLE_LOGGER: Cell<Option<&'static dyn JsLog>> = Cell::new(None)
}
pub fn set_js_logger(logger: &'static dyn JsLog) {
    CONSOLE_LOGGER.set(Some(logger));
}

pub(crate) fn log(log_data: LogData, context: &mut Context<'_>) -> JsResult<()> {
    CONSOLE_LOGGER.with(|logger| {
        if let Some(logger) = logger.get() {
            logger.log(log_data, context);
            Ok(())
        } else {
            Err(JsNativeError::eval()
                .with_message("JS_LOGGER not set")
                .into())
        }
    })
}
