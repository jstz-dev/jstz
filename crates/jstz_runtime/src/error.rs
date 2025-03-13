use deno_core::{
    error::{CoreError, JsError},
    serde_v8, v8,
};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    DenoCore(#[from] CoreError),
    #[error(transparent)]
    SerdeV8(#[from] serde_v8::Error),
}

impl From<v8::DataError> for RuntimeError {
    fn from(data_error: v8::DataError) -> Self {
        Self::DenoCore(data_error.into())
    }
}

impl From<JsError> for RuntimeError {
    fn from(js_error: JsError) -> Self {
        Self::DenoCore(js_error.into())
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
