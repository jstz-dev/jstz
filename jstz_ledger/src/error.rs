use derive_more::{Display, Error, From};
#[derive(Display, Debug, Error, From)]
pub enum Error {
    CoreError { source: jstz_core::Error },
    BalanceOverflow,
}

impl From<tezos_smart_rollup_host::path::PathError> for Error {
    fn from(source: tezos_smart_rollup_host::path::PathError) -> Self {
        Error::CoreError {
            source: jstz_core::Error::PathError { source },
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
