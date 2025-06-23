use std::borrow::Cow;

use deno_core::{
    error::{CoreError, JsError},
    serde_v8, v8,
};
use deno_error::JsErrorBox;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error(transparent)]
pub enum RuntimeError {
    #[class(inherit)]
    DenoCore(#[from] CoreError),
    #[class(inherit)]
    SerdeV8(#[from] serde_v8::Error),
    #[class(inherit)]
    WebSysError(#[from] WebSysError),
    #[class(generic)]
    #[error("Execution deadline exceeded")]
    DeadlineExceeded,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class("WebSysError")]
pub enum WebSysError {
    #[error("Class `{0}` not found in `globalThis`")]
    ClassMissing(String),
    #[error("Failed to construct instance of class `{0}`")]
    ConstructorFailed(String),
    #[error("Method `{method_name}` not found on class `{class_name}`")]
    MethodMissing {
        class_name: String,
        method_name: String,
    },
    #[error("Call to method `{0}` failed")]
    MethodCallFailed(String),
    #[error("Property `{0}` not found")]
    PropertyMissing(String),
    #[error("Failed to set the property `{0}`")]
    PropertySetFailed(String),
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

impl RuntimeError {
    pub fn type_error(message: impl Into<Cow<'static, str>>) -> Self {
        Self::DenoCore(JsErrorBox::type_error(message).into())
    }

    pub fn cannot_alloc(ty: impl Into<Cow<'static, str>>) -> Self {
        Self::DenoCore(
            JsErrorBox::generic(format!("Cannot allocate `{}`", ty.into())).into(),
        )
    }
}
