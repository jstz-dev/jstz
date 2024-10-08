use boa_engine::{JsError, JsNativeError};
use derive_more::{Display, Error, From};

#[derive(Display, Debug, Error, From)]
pub enum KvError {
    DowncastFailed,
    TransactionStackEmpty,
    ExpectedLookupMapEntry,
}

#[derive(Display, Debug, Error, From)]
pub enum Error {
    KvError {
        source: KvError,
    },
    HostError {
        source: crate::host::HostError,
    },
    PathError {
        source: tezos_smart_rollup_host::path::PathError,
    },
    JsError {
        source: JsError,
    },
    SerializationError {
        description: String,
    },
    OutboxError {
        source: crate::kv::outbox::OutboxError,
    },
}

impl From<Error> for JsError {
    fn from(value: Error) -> Self {
        match value {
            Error::KvError { source } => JsNativeError::eval()
                .with_message(format!("KvError: {}", source))
                .into(),
            Error::HostError { source } => JsNativeError::eval()
                .with_message(format!("HostError: {}", source))
                .into(),
            Error::PathError { source } => JsNativeError::eval()
                .with_message(format!("PathError: {}", source))
                .into(),
            Error::JsError { source } => JsNativeError::eval()
                .with_message("JsError")
                .with_cause(source)
                .into(),
            Error::SerializationError { description } => JsNativeError::eval()
                .with_message(format!("serialization error: {description}"))
                .into(),
            Error::OutboxError { source } => JsNativeError::eval()
                .with_message(format!("OutboxError: {}", source))
                .into(),
        }
    }
}

impl From<boa_engine::JsNativeError> for Error {
    fn from(source: boa_engine::JsNativeError) -> Self {
        Error::JsError {
            source: source.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
