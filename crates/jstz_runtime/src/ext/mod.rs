pub(crate) mod jstz_console;
pub(crate) mod jstz_fetch;
pub mod jstz_kv;

pub mod jstz_test;

pub(crate) mod jstz_main;

pub use jstz_fetch::FetchHandlerOptions;

#[derive(Debug, ::thiserror::Error, deno_error::JsError)]
#[class(not_supported)]
#[error("{name} is not supported")]
pub struct NotSupported {
    pub name: &'static str,
}
