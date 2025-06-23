#![allow(dead_code)]
use crate::sequencer::db::Db;
use log::error;

use thiserror::Error;

pub type BlockLevel = u32;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {0}")]
    DbError(#[from] anyhow::Error),
}

pub trait CheckpointStore {
    fn get_checkpoint(&self) -> Result<Option<BlockLevel>>;
    fn set_checkpoint(&self, level: BlockLevel) -> Result<()>;
}

/// Tracks the checkpoint level, which is the last block level that has been pushed into the queue.
#[derive(Clone)]
pub struct InboxCheckpoint {
    db: Db,
}

impl InboxCheckpoint {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

const LAST_LEVEL: &str = "/inbox/last_level";
impl CheckpointStore for InboxCheckpoint {
    fn get_checkpoint(&self) -> Result<Option<BlockLevel>> {
        let value = self.db.read_key(LAST_LEVEL)?;
        Ok(value.map(|v| v.parse::<BlockLevel>().unwrap()))
    }

    fn set_checkpoint(&self, level: BlockLevel) -> Result<()> {
        self.db.write(LAST_LEVEL, &level.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sequencer::{
        db::Db,
        inbox::store::{CheckpointStore, InboxCheckpoint},
    };

    #[test]
    fn checkpoint_store() {
        let db = Db::init(Some("")).unwrap();
        let store = InboxCheckpoint::new(db);
        assert_eq!(store.get_checkpoint().unwrap(), None);
        store.set_checkpoint(1).unwrap();
        assert_eq!(store.get_checkpoint().unwrap(), Some(1));
    }
}
