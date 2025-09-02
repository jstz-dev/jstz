use boa_engine::{Context, JsNativeError, JsResult};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    fmt::{self, Display},
};
use utoipa::ToSchema;

#[derive(
    Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug, ValueEnum, ToSchema,
)]
pub enum LogLevel {
    ERROR = 1,
    WARN = 2,
    INFO = 3,
    DEBUG = 4,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::ERROR => write!(f, "ERROR"),
            LogLevel::WARN => write!(f, "WARN"),
            LogLevel::INFO => write!(f, "INFO"),
            LogLevel::DEBUG => write!(f, "DEBUG"),
        }
    }
}

impl TryFrom<&str> for LogLevel {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ERROR" => Ok(LogLevel::ERROR),
            "WARN" => Ok(LogLevel::WARN),
            "INFO" => Ok(LogLevel::INFO),
            "DEBUG" => Ok(LogLevel::DEBUG),
            _ => Err(format!("Invalid LogLevel: {value}")),
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
    fn log(&self, log_data: LogData, context: &mut Context);
    fn flush(&self) {}
}

thread_local! {
    /// Thread-local logger
    static CONSOLE_LOGGER: Cell<Option<&'static dyn JsLog>> = Cell::new(None)
}

pub fn set_js_logger(logger: &'static dyn JsLog) {
    CONSOLE_LOGGER.set(Some(logger));
}

pub(crate) fn log(log_data: LogData, context: &mut Context) -> JsResult<()> {
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
