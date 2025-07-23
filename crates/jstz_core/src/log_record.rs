use std::fmt::{self, Display};

use clap::ValueEnum;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const LOG_PREFIX: &str = "[JSTZ:SMART_FUNCTION:LOG] ";

#[derive(Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub address: SmartFunctionHash,
    pub request_id: String,
    pub level: LogLevel,
    pub text: String,
}

impl Display for LogRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &serde_json::to_string(self).expect("Failed to convert LogRecord to string"),
        )
    }
}

impl LogRecord {
    pub fn try_from_string(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

#[derive(
    Serialize, Deserialize, PartialEq, PartialOrd, Clone, Debug, ValueEnum, ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
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

    fn try_from(value: &str) -> Result<Self, <Self as TryFrom<&str>>::Error> {
        match value {
            "ERROR" => Ok(LogLevel::ERROR),
            "WARN" => Ok(LogLevel::WARN),
            "INFO" => Ok(LogLevel::INFO),
            "DEBUG" => Ok(LogLevel::DEBUG),
            _ => Err(format!("Invalid LogLevel: {value}")),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct LogData {
    pub level: LogLevel,
    pub text: String,
    pub groups_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};

    fn dummy_hash() -> SmartFunctionHash {
        // Replace with a valid SmartFunctionHash constructor as needed
        SmartFunctionHash::digest(&[0u8; 32]).unwrap()
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::ERROR.to_string(), "ERROR");
        assert_eq!(LogLevel::WARN.to_string(), "WARN");
        assert_eq!(LogLevel::INFO.to_string(), "INFO");
        assert_eq!(LogLevel::DEBUG.to_string(), "DEBUG");
    }

    #[test]
    fn test_log_level_try_from_str() {
        assert_eq!(LogLevel::try_from("ERROR").unwrap(), LogLevel::ERROR);
        assert_eq!(LogLevel::try_from("WARN").unwrap(), LogLevel::WARN);
        assert_eq!(LogLevel::try_from("INFO").unwrap(), LogLevel::INFO);
        assert_eq!(LogLevel::try_from("DEBUG").unwrap(), LogLevel::DEBUG);
        assert!(LogLevel::try_from("INVALID").is_err());
    }

    #[test]
    fn test_log_record() {
        let record = LogRecord {
            address: dummy_hash(),
            request_id: "req-123".to_string(),
            level: LogLevel::INFO,
            text: "Hello, world!".to_string(),
        };
        let s = record.to_string();

        let parsed: LogRecord = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.request_id, "req-123");
        assert_eq!(parsed.level, LogLevel::INFO);
        assert_eq!(parsed.text, "Hello, world!");

        let opt = LogRecord::try_from_string(&s);
        assert!(opt.is_some());
        let rec2 = opt.unwrap();
        assert_eq!(rec2.request_id, "req-123");
        assert_eq!(rec2.level, LogLevel::INFO);
        assert_eq!(rec2.text, "Hello, world!");

        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(json, "{\"address\":\"KT18mgybN9E97hF9HG9cDfSz6ofT7w9WTzMH\",\"requestId\":\"req-123\",\"level\":\"INFO\",\"text\":\"Hello, world!\"}");
        let de: LogRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, de);
    }

    #[test]
    fn test_log_data_serde() {
        let data = LogData {
            level: LogLevel::WARN,
            text: "Warn message".to_string(),
            groups_len: 2,
        };
        let json = serde_json::to_string(&data).unwrap();
        assert_eq!(
            json,
            "{\"level\":\"WARN\",\"text\":\"Warn message\",\"groups_len\":2}"
        );
        let de: LogData = serde_json::from_str(&json).unwrap();
        assert_eq!(de, data);
    }
}
