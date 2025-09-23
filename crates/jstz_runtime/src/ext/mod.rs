pub(crate) mod jstz_console;
pub(crate) mod jstz_fetch;
pub mod jstz_kv;
pub(crate) mod jstz_main;

pub use jstz_fetch::FetchHandlerOptions;

#[derive(Debug, ::thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("{name} is not supported")]
pub struct NotSupported {
    pub name: &'static str,
}
