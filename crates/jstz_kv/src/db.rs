//! This module defines a database structure that manages the global
//! state of the KV store.

use std::sync::Arc;

use crate::mem_table::MemTable;

/// An entry value is either a tombstone or a value.
pub type EntryValue<V> = Option<V>;

/// Represents a database with a global memtable.
#[derive(Debug, Clone)]
pub struct Db<Id: Ord, K, V> {
    pub(crate) mem_table: Arc<MemTable<Id, K, V>>,
}

impl<Id: Ord, K, V> Default for Db<Id, K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Ord, K, V> Db<Id, K, V> {
    /// Creates a new database with an empty memtable.
    pub fn new() -> Self {
        Self {
            mem_table: Arc::new(MemTable::new()),
        }
    }
}
