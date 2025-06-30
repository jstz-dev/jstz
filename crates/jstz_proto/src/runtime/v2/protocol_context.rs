use std::sync::{Arc, OnceLock};

use jstz_core::host::HostRuntime;
use parking_lot::Mutex;

use super::oracle::{Oracle, OracleError};

/// Holds stateful globals required by the protocol
pub static PROTOCOL_CONTEXT: OnceLock<Arc<Mutex<ProtocolContext>>> = OnceLock::new();

pub struct ProtocolContext {
    oracle: Oracle,
}

impl ProtocolContext {
    pub fn oracle(&mut self) -> &mut Oracle {
        &mut self.oracle
    }

    /// Initialize the global protocol context
    pub fn init_global(rt: &mut impl HostRuntime) -> Result<(), ProtocolContextError> {
        let oracle = Oracle::new(rt, None)?;
        PROTOCOL_CONTEXT.get_or_init(|| Arc::new(Mutex::new(ProtocolContext { oracle })));
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolContextError {
    #[error(transparent)]
    OracleFailedToInitialize(#[from] OracleError),
}
