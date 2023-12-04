use boa_engine::{JsError, JsNativeError};
use derive_more::{Display, Error, From};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    HostError {
        source: crate::host::HostError,
    },
    PathError {
        source: tezos_smart_rollup_host::path::PathError,
    },
    JsError {
        source: JsError,
    },
}

impl From<Error> for JsError {
    fn from(value: Error) -> Self {
        match value {
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
