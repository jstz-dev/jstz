//! # Optimistic transactional key-value store
//!
//! This module provides a persistent transactional key-value store.

use boa_gc::{Finalize, Trace};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};
use tezos_smart_rollup_host::runtime::ValueType;
use tezos_smart_rollup_host::{path::Path, runtime::Runtime};

use crate::error::Result;
use crate::event::Event;

pub mod outbox;
pub mod transaction;
pub mod value;

pub use transaction::{Entry, JsTransaction, Transaction};
pub use value::Value;

/// A transactional key-value store using an optimistic concurrency control scheme.
///
/// Reads and writes 'serde' values with path-like keys. All reads/writes happen through
/// transactions.
///
/// Transactions are implemented using 'optimistic concurrency control'. The approach
/// is rooted in the notion that conflicts between transactions are infrequent, therefore
/// we *optimistically* assume that conflicts won't happen. A transaction is split into
/// three phases:
///
///   - _read phase_: during which a transaction reads and writes values to a local
///     snapshot.
///
///   - _validation phase_: during which a transaction checks to see if its execution is
///     consistent with a serialization of recently committed concurrently executing
///     transactions.                
///
///   - _commit phase_: during which the transaction flushes its local snapshot into
///     the persistent store.
///

#[derive(Trace, Finalize)]
pub struct Storage;

impl Storage {
    /// Retrieve a value from the persistent store if it exists
    pub fn get<V: Value>(rt: &impl Runtime, key: &impl Path) -> Result<Option<V>> {
        match rt.store_has(key)? {
            Some(ValueType::Value | ValueType::ValueWithSubtree) => {
                let bytes = rt.store_read_all(key)?;
                let value = V::decode(&bytes)?;
                Ok(Some(value))
            }
            _ => Ok(None),
        }
    }

    /// Returns `true` if the persistent store contains a key-value pair for the
    /// specified key.
    pub fn contains_key(rt: &impl Runtime, key: &impl Path) -> Result<bool> {
        match rt.store_has(key)? {
            Some(ValueType::Value | ValueType::ValueWithSubtree) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Insert a key-value pair into the persistent store
    pub fn insert<V: Value + ?Sized>(
        rt: &mut impl Runtime,
        key: &impl Path,
        value: &V,
    ) -> Result<()> {
        rt.store_write(key, value.encode()?.as_slice(), 0)?;
        Ok(())
    }

    /// Remove a key-value pair from the persistent store
    pub fn remove(rt: &mut impl Runtime, key: &impl Path) -> Result<()> {
        if Self::contains_key(rt, key)? {
            rt.store_delete(key)?;
        }
        Ok(())
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum StorageUpdate {
    /// Upsert a value at the given key.
    Insert {
        key: String,
        #[serde_as(as = "Base64")]
        value: Vec<u8>,
    },
    /// Remove the value at the given key.
    Remove { key: String },
}

/// A storage updates event.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BatchStorageUpdate(Vec<StorageUpdate>);

impl Event for BatchStorageUpdate {
    fn tag() -> &'static str {
        "BATCH_STORAGE_UPDATE"
    }
}

impl BatchStorageUpdate {
    pub fn new(size: usize) -> Self {
        Self(Vec::with_capacity(size))
    }

    pub fn push_insert<K: Path, V: Value + ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<()> {
        self.0.push(StorageUpdate::Insert {
            key: key.to_string(),
            value: value.encode()?,
        });
        Ok(())
    }

    pub fn push_remove<K: Path>(&mut self, key: &K) {
        self.0.push(StorageUpdate::Remove {
            key: key.to_string(),
        });
    }

    pub fn into_vec(self) -> Vec<StorageUpdate> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl IntoIterator for BatchStorageUpdate {
    type Item = StorageUpdate;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
