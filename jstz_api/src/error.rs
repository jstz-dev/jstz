use crate::conversion::ToJsError;
use boa_engine::{Context, JsError, JsNativeError};
use derive_more::{Display, Error, From};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    JsError { source: boa_engine::JsError },
    CoreError { source: jstz_core::Error },
    CryptoError { source: jstz_crypto::Error },
    LedgerError { source: jstz_ledger::Error },
}

impl From<Error> for JsError {
    fn from(value: Error) -> Self {
        match value {
            Error::JsError { source } => source,
            Error::CoreError { source } => JsNativeError::eval()
                .with_message(source.to_string())
                .into(),
            Error::CryptoError { source } => JsNativeError::eval()
                .with_message(source.to_string())
                .into(),
            Error::LedgerError { source } => JsNativeError::eval()
                .with_message(source.to_string())
                .into(),
        }
    }
}
impl ToJsError for Error {
    fn to_js_error(self, _context: &mut Context) -> JsError {
        self.into()
    }
}

impl From<JsNativeError> for Error {
    fn from(value: JsNativeError) -> Self {
        Error::JsError {
            source: value.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
