//! # Optimistic transactional key-value store
//!
//! This module provides a persistent transactional key-value store.

use std::collections::BTreeSet;

use boa_gc::{empty_trace, Finalize, Trace};
use serde::de::DeserializeOwned;
use tezos_smart_rollup_host::runtime::ValueType;
use tezos_smart_rollup_host::{
    path::{OwnedPath, Path},
    runtime::Runtime,
};

use crate::error::Result;

mod transaction;
pub mod value;

pub use transaction::{Entry, Transaction};
pub use value::Value;

const MAX_TX_COUNT: usize = 16;

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
    pub fn get<V>(rt: &impl Runtime, key: &impl Path) -> Result<Option<V>>
    where
        V: Value + DeserializeOwned,
    {
        match rt.store_has(key)? {
            Some(ValueType::Value | ValueType::ValueWithSubtree) => {
                let bytes = rt.store_read_all(key)?;
                let value = value::deserialize(&bytes);
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
    pub fn insert<V>(rt: &mut impl Runtime, key: &impl Path, value: &V) -> Result<()>
    where
        V: Value + ?Sized,
    {
        rt.store_write(key, &value::serialize(value), 0)?;
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

#[derive(Default)]
pub struct Kv {
    clock: Clock,
    update_sets: [BTreeSet<OwnedPath>; MAX_TX_COUNT],
}

impl Finalize for Kv {}

unsafe impl Trace for Kv {
    empty_trace!();
}

impl Kv {
    /// Create an in-memory representation of the persistent key-value store
    /// with a given _scheme_.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new transaction.
    pub fn begin_transaction(&self) -> Transaction {
        Transaction::new(self.clock.current_timestamp())
    }

    /// Commit a transaction. Returns `true` if the transaction was successfully
    /// committed to the persistent key-value store.
    pub fn commit_transaction(
        &mut self,
        rt: &mut impl Runtime,
        tx: Transaction,
    ) -> Result<bool> {
        // Transactions are (optimistically) assigned a timestamp when they
        // enter the validation phase.
        let possible_commit_timestamp = self.clock.current_timestamp() + 1;

        // **Validation Phase**
        //
        // A transaction at timestamp `j` is 'valid' if the following holds:
        //  for all transactions `i` < `j`:
        //      - if the update set of `i` overlaps the read set of `j`:
        //          transaction `i` must finish its write phase before
        //          transaction `j` starts its read phase
        //      - if the update set of `i` overlaps the update set of `j`:
        //          transaction `i` must finish its write phase before transaction `j`
        //          starts its write phase
        //      - else:
        //          the transaction can overlap arbitrarily

        let read_set = tx.read_set();
        for ts in tx.begin_timestamp + 1..possible_commit_timestamp {
            let update_set = &self.update_sets[(ts as usize) % MAX_TX_COUNT];
            if read_set.intersection(update_set).count() > 0 {
                return Ok(false);
            }
        }

        // **Commit Phase**
        //
        // The transaction `tx` has now been verified at this point. We can assign it a `commit_timestamp`
        // by stepping the store's (lamport) clock. The `update_set` of `tx` is recorded in the
        // store's recently committed transaction `update_sets`. After this, we can safetly flush
        // the transaction's local snapshot to the persistent store.

        let commit_timestamp = self.clock.next_timestamp();
        let update_set = tx.update_set();
        self.update_sets[(commit_timestamp as usize) % MAX_TX_COUNT] = update_set;
        tx.flush(rt)?;

        Ok(true)
    }

    /// Rollback a transaction.
    pub fn rollback_transaction(&mut self, _: &mut impl Runtime, tx: Transaction) {
        drop(tx)
    }
}

type Timestamp = u64;

// A simple (not atomic) lamport clock
#[derive(Debug, Default)]
struct Clock {
    counter: Timestamp,
}

impl Clock {
    fn current_timestamp(&self) -> Timestamp {
        self.counter
    }

    fn next_timestamp(&mut self) -> Timestamp {
        let timestamp = self.counter;
        self.counter += 1;
        timestamp
    }
}
