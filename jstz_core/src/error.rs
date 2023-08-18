use derive_more::{Display, Error, From};

#[derive(Display, Debug, Error, From)]
pub enum Error {
    HostError {
        source: crate::host::HostError,
    },
    PathError {
        source: tezos_smart_rollup_host::path::PathError,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
