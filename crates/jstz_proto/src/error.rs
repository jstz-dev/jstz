use boa_engine::{JsError, JsNativeError};
use derive_more::{Display, Error, From};
use tezos_smart_rollup::michelson::ticket::TicketHashError;

use crate::{
    context::ticket_table,
    executor::{fa_deposit, fa_withdraw},
};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    CoreError {
        source: jstz_core::Error,
    },
    CryptoError {
        source: jstz_crypto::Error,
    },
    AccountDoesNotExist,
    BalanceOverflow,
    InsufficientFunds,
    InvalidNonce,
    InvalidAddress,
    InvalidScheme,
    RefererShouldNotBeSet,
    GasLimitExceeded,
    UnsupportedPath,
    InvalidHost,
    InvalidHttpRequest,
    InvalidHttpRequestBody,
    InvalidHttpRequestMethod,
    InvalidHeaderValue,
    InvalidUri,
    InvalidTicketType,
    TicketTableError {
        source: ticket_table::TicketTableError,
    },
    FaDepositError {
        source: fa_deposit::FaDepositError,
    },
    FaWithdrawError {
        source: fa_withdraw::FaWithdrawError,
    },
    TicketHashError(TicketHashError),
    TicketAmountTooLarge,
    ZeroAmountNotAllowed,
    AddressTypeMismatch,
    AccountExists,
    RevealTypeMismatch,
    RevealNotSupported,
    InvalidInjector,
    #[cfg(feature = "v2_runtime")]
    V2Error(crate::runtime::v2::Error),
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
            Error::InsufficientFunds => JsNativeError::eval()
                .with_message("InsufficientFunds")
                .into(),
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
            Error::InvalidHttpRequestBody => JsNativeError::eval()
                .with_message("InvalidHttpRequestBody")
                .into(),
            Error::InvalidHttpRequestMethod => JsNativeError::eval()
                .with_message("InvalidHttpRequestMethod")
                .into(),
            Error::InvalidHost => {
                JsNativeError::eval().with_message("InvalidHost").into()
            }
            Error::UnsupportedPath => {
                JsNativeError::eval().with_message("UnsupportedPath").into()
            }
            Error::TicketTableError { source } => JsNativeError::eval()
                .with_message(format!("TicketTableError: {}", source))
                .into(),
            Error::FaDepositError { source } => JsNativeError::eval()
                .with_message(format!("FaDepositError: {}", source))
                .into(),
            Error::FaWithdrawError { source } => JsNativeError::eval()
                .with_message(format!("FaWithdrawError: {}", source))
                .into(),
            Error::TicketHashError(inner) => JsNativeError::eval()
                .with_message(format!("{}", inner))
                .into(),
            Error::TicketAmountTooLarge => JsNativeError::eval()
                .with_message("TicketAmountTooLarge")
                .into(),
            Error::InvalidTicketType => JsNativeError::eval()
                .with_message("InvalidTicketType")
                .into(),
            Error::InvalidUri => JsNativeError::eval().with_message("InvalidUri").into(),
            Error::InvalidHeaderValue => JsNativeError::eval()
                .with_message("InvalidHeaderValue")
                .into(),
            Error::ZeroAmountNotAllowed => JsNativeError::eval()
                .with_message("ZeroAmountNotAllowed")
                .into(),
            Error::AddressTypeMismatch => JsNativeError::eval()
                .with_message("AddressTypeMismatch")
                .into(),
            Error::AccountExists => {
                JsNativeError::eval().with_message("AccountExists").into()
            }
            Error::RevealTypeMismatch => JsNativeError::eval()
                .with_message("RevealTypeMismatch")
                .into(),
            Error::RevealNotSupported => JsNativeError::eval()
                .with_message("RevealNotSupported")
                .into(),
            Error::InvalidInjector => {
                JsNativeError::eval().with_message("InvalidInjector").into()
            }
            Error::InvalidScheme => {
                JsNativeError::eval().with_message("InvalidScheme").into()
            }
            Error::AccountDoesNotExist => JsNativeError::eval()
                .with_message("AccountDoesNotExist")
                .into(),
            #[cfg(feature = "v2_runtime")]
            Error::V2Error(_) => {
                unimplemented!("V2 runtime errors are not supported in boa")
            }
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
