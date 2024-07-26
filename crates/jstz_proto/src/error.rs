use boa_engine::{JsError, JsNativeError};
use derive_more::{Display, Error, From};

use crate::context::ticket_table;

#[derive(Display, Debug, Error, From)]
pub enum Error {
    CoreError {
        source: jstz_core::Error,
    },
    CryptoError {
        source: jstz_crypto::Error,
    },
    BalanceOverflow,
    InvalidNonce,
    InvalidAddress,
    RefererShouldNotBeSet,
    GasLimitExceeded,
    InvalidHttpRequest,
    TicketTableError {
        source: ticket_table::TicketTableError,
    },
}
pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for JsError {
    fn from(value: Error) -> Self {
        match value {
            Error::CoreError { source } => source.into(),
            Error::CryptoError { source } => JsNativeError::eval()
                .with_message(format!("CryptoError: {}", source))
                .into(),
            Error::BalanceOverflow => {
                JsNativeError::eval().with_message("BalanceOverflow").into()
            }
            Error::InvalidNonce => {
                JsNativeError::eval().with_message("InvalidNonce").into()
            }
            Error::InvalidAddress => {
                JsNativeError::eval().with_message("InvalidAddress").into()
            }
            Error::RefererShouldNotBeSet => JsNativeError::eval()
                .with_message("RefererShouldNotBeSet")
                .into(),
            Error::GasLimitExceeded => JsNativeError::eval()
                .with_message("GasLimitExceeded")
                .into(),
            Error::InvalidHttpRequest => JsNativeError::eval()
                .with_message("InvalidHttpRequest")
                .into(),
            Error::TicketTableError { source } => JsNativeError::eval()
                .with_message(format!("TicketTableError: {}", source))
                .into(),
        }
    }
}

impl From<boa_engine::JsNativeError> for Error {
    fn from(source: boa_engine::JsNativeError) -> Self {
        Error::CoreError {
            source: source.into(),
        }
    }
}

impl From<boa_engine::JsError> for Error {
    fn from(source: boa_engine::JsError) -> Self {
        Error::CoreError {
            source: jstz_core::Error::JsError { source },
        }
    }
}

impl From<tezos_smart_rollup::storage::path::PathError> for Error {
    fn from(source: tezos_smart_rollup::storage::path::PathError) -> Self {
        Error::CoreError {
            source: jstz_core::Error::PathError { source },
        }
    }
}
