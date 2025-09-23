use std::borrow::Cow;

use deno_error::JsErrorClass as _;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use jstz_runtime::error::RuntimeError;
use serde::Serialize;

use crate::runtime::v2::oracle::OracleError;

use super::http::*;

pub type Result<T> = std::result::Result<T, FetchError>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FetchError {
    #[class(type)]
    #[error("Invalid Header type")]
    InvalidHeaderType,
    #[class(type)]
    #[error("Unsupported scheme '{0}'")]
    UnsupportedScheme(String),
    #[class(uri)]
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[class(type)]
    #[error("Invalid Response type")]
    InvalidResponseType,
    #[class(generic)]
    #[error(transparent)]
    RuntimeError(#[from] RuntimeError),
    #[class(generic)]
    #[error("NotSupportedError:{0}")]
    NotSupported(&'static str),
    #[class(generic)]
    #[error("Oracle calls are not allowed to be called from RunFunction")]
    TopLevelOracleCallNotSupported,
    #[class(inherit)]
    #[error(transparent)]
    OracleError(#[from] OracleError),
    // TODO: Boa's JsClass errors are not Send safe. Once we remove boa, we
    // should be able to use crate::Error type directly
    #[class(generic)]
    #[error("JstzError: {0}")]
    JstzError(String),
    #[class(syntax)]
    #[error("Smart function '{address}' has no code")]
    EmptyCode { address: SmartFunctionHash },
}

#[derive(Serialize)]
pub struct FetchErrorJsClass {
    class: Cow<'static, str>,
    message: Option<Cow<'static, str>>,
}

impl From<FetchError> for FetchErrorJsClass {
    fn from(value: FetchError) -> Self {
        Self {
            class: value.get_class(),
            message: Some(value.get_message()),
        }
    }
}

impl From<FetchError> for Response {
    fn from(err: FetchError) -> Self {
        let error_body: FetchErrorJsClass = err.into();
        let error = serde_json::to_vec(&error_body)
            .map(Body::Vector)
            .ok()
            .unwrap_or(Body::zero_capacity());
        Response {
            status: 500,
            status_text: "InternalServerError".to_string(),
            headers: Vec::with_capacity(0),
            body: error,
        }
    }
}

impl From<Result<Response>> for Response {
    fn from(result: Result<Response>) -> Self {
        match result {
            Ok(response) => response,
            Err(err) => err.into(),
        }
    }
}
