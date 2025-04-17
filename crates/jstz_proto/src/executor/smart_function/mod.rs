pub(crate) mod deploy;
pub(crate) mod host_script;
pub(crate) mod run;
pub(crate) mod script;

pub use host_script::{JSTZ_HOST, X_JSTZ_AMOUNT, X_JSTZ_TRANSFER};
pub use run::NOOP_PATH;
pub use script::{register_jstz_apis, register_web_apis};

pub use deploy::deploy_smart_function as deploy;
