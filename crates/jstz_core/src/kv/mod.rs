//! # Optimistic transactional key-value store
//!
//! This module provides a persistent transactional key-value store.

use boa_gc::{Finalize, Trace};
use tezos_smart_rollup_host::runtime::ValueType;
use tezos_smart_rollup_host::{path::Path, runtime::Runtime};

use crate::error::Result;

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
