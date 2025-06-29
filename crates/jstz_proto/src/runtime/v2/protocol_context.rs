use std::sync::{Arc, OnceLock};

use jstz_core::host::HostRuntime;
use parking_lot::Mutex;

use crate::BlockLevel;

use super::oracle::{Oracle, OracleError};

/// Holds stateful globals required by the protocol
pub static PROTOCOL_CONTEXT: OnceLock<ProtocolContext> = OnceLock::new();

pub struct ProtocolContext {
    oracle: Arc<Mutex<Oracle>>,
    current_level: Arc<Mutex<BlockLevel>>,
}

impl ProtocolContext {
    pub fn oracle(&self) -> Arc<Mutex<Oracle>> {
        self.oracle.clone()
    }

    pub fn current_level(&self) -> BlockLevel {
        let level = self.current_level.lock();
        *level
    }

    #[cfg(test)]
    pub fn set_level(&self, new_level: BlockLevel) {
        let mut level = self.current_level.lock();
        *level = new_level
    }

    /// Initialize the global protocol context
    pub fn init_global(
        rt: &mut impl HostRuntime,
        current_level: BlockLevel,
    ) -> Result<(), ProtocolContextError> {
        let current_level = Arc::new(Mutex::new(current_level));
        let oracle = Oracle::new(rt, None)?;
        PROTOCOL_CONTEXT.get_or_init(|| ProtocolContext {
            oracle: Arc::new(Mutex::new(oracle)),
            current_level,
        });
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolContextError {
    #[error(transparent)]
    OracleFailedToInitialize(#[from] OracleError),
}
