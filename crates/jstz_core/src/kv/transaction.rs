use std::{
    collections::{btree_map, BTreeMap, BTreeSet},
    marker::PhantomData,
};

use boa_gc::{empty_trace, Finalize, Trace};
use serde::de::DeserializeOwned;
use tezos_smart_rollup_host::{path::OwnedPath, runtime::Runtime};

use crate::error::Result;

use super::value::{BoxedValue, Value};
use super::{Storage, Timestamp};

/// A transaction is a 'lazy' snapshot of the persistent key-value store from
/// the point in time when the transaction began. Modifications to new or old
/// values within the transaction remain isolated from any concurrent
/// transactions.
///
/// Reads are cached for each transaction, optimizing repeated accesses to the
/// same key. Writes are buffered in an in-memory `BTreeMap` until the
/// transaction is successfully committed, at which point the buffer is flushed
/// to the persistent storage.
///
/// Transactions offer ACID guarentees. The weakest property for these gaurentees
/// to hold is [serializability](https://en.wikipedia.org/wiki/Serializability), ensuring
/// that a transaction can only be committed if it does not conflict with a
/// previously committed transaction. For example, if a transaction `t1` reads any key-value
/// pair that is modified and committed in a later transaction `t2` before `t1` is comitted,
/// `t1` will fail. In other words, the following transaction behaviour will lead to a
/// conflict:
///
/// ```text
/// +- t1: ---------+
/// | read key1     |   +- t2 ----------+
/// |               |   | write key1    |
/// |               |   | commit: true  |
/// | write key1    |   +---------------+
/// | commit: false |
/// +---------------+
/// ```

#[must_use]
pub struct Transaction {
    remove_set: BTreeSet<OwnedPath>,
    snapshot: Snapshot,
    pub(crate) begin_timestamp: Timestamp,
}

impl Finalize for Transaction {}

unsafe impl Trace for Transaction {
    empty_trace!();
}

struct SnapshotEntry {
    dirty: bool,
    value: BoxedValue,
}

type Snapshot = BTreeMap<OwnedPath, SnapshotEntry>;

impl SnapshotEntry {
    fn ephemeral<V>(value: V) -> Self
    where
        V: Value,
    {
        Self {
            dirty: true,
            value: BoxedValue::new(value),
        }
    }

    fn persistent<V>(value: V) -> Self
    where
        V: Value,
    {
        Self {
            dirty: false,
            value: BoxedValue::new(value),
        }
    }

    fn as_ref<V>(&self) -> &V
    where
        V: Value,
    {
        self.value.as_any().downcast_ref().unwrap()
    }

    fn as_mut<V>(&mut self) -> &mut V
    where
        V: Value,
    {
        self.dirty = true;
        self.value.as_any_mut().downcast_mut().unwrap()
    }

    fn into_value<V>(self) -> V
    where
        V: Value,
    {
        let value = self.value.downcast().unwrap();
        *value
    }
}

impl Transaction {
    pub(crate) fn new(begin_timestamp: Timestamp) -> Self {
        Self {
            begin_timestamp,
            remove_set: BTreeSet::new(),
            snapshot: BTreeMap::new(),
        }
    }

    pub(crate) fn read_set(&self) -> BTreeSet<OwnedPath> {
        self.snapshot
            .iter()
            .filter_map(
                |(k, entry)| {
                    if !entry.dirty {
                        Some(k.clone())
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    fn insert_set(&self) -> BTreeSet<OwnedPath> {
        self.snapshot
            .iter()
            .filter_map(
                |(k, entry)| {
                    if entry.dirty {
                        Some(k.clone())
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    pub(crate) fn update_set(&self) -> BTreeSet<OwnedPath> {
        self.insert_set().union(&self.remove_set).cloned().collect()
    }

    pub(crate) fn flush(self, rt: &mut impl Runtime) -> Result<()> {
        // Perform deletions
        for key in self.remove_set {
            Storage::remove(rt, &key)?
        }

        // Perform insertions
        for (key, entry) in self.snapshot.into_iter() {
            if entry.dirty {
                Storage::insert(rt, &key, entry.value.as_ref())?;
            }
        }

        Ok(())
    }

    fn lookup<'a, V>(
        &'a mut self,
        rt: &impl Runtime,
        key: OwnedPath,
    ) -> Result<Option<&'a mut SnapshotEntry>>
    where
        V: Value + DeserializeOwned,
    {
        let entry = self.snapshot.entry(key);

        match entry {
            btree_map::Entry::Vacant(entry) => {
                if let Some(value) = Storage::get::<V>(rt, entry.key())? {
                    let snapshot_entry = entry.insert(SnapshotEntry::persistent(value));

                    return Ok(Some(snapshot_entry));
                }

                Ok(None)
            }
            btree_map::Entry::Occupied(entry) => Ok(Some(entry.into_mut())),
        }
    }

    /// Returns a reference to the value corresponding to the key in the
    /// key-value store if it exists.
    pub fn get<V>(&mut self, rt: &impl Runtime, key: OwnedPath) -> Result<Option<&V>>
    where
        V: Value + DeserializeOwned,
    {
        self.lookup::<V>(rt, key)
            .map(|entry_opt| entry_opt.map(|entry| entry.as_ref()))
    }

    /// Returns a mutable reference to the value corresponding to the key in the
    /// key-value store if it exists.
    pub fn get_mut<V>(
        &mut self,
        rt: &impl Runtime,
        key: OwnedPath,
    ) -> Result<Option<&mut V>>
    where
        V: Value + DeserializeOwned,
    {
        self.lookup::<V>(rt, key)
            .map(|entry_opt| entry_opt.map(|entry| entry.as_mut()))
    }

    /// Returns `true` if the key-value store contains a key-value pair for the
    /// specified key.
    pub fn contains_key(&self, rt: &impl Runtime, key: &OwnedPath) -> Result<bool> {
        Ok(self.snapshot.contains_key(key) || Storage::contains_key(rt, key)?)
    }

    /// Insert a key-value pair into the key-value store.
    pub fn insert<V>(&mut self, key: OwnedPath, value: V) -> Result<()>
    where
        V: Value,
    {
        self.snapshot.insert(key, SnapshotEntry::ephemeral(value));
        Ok(())
    }

    /// Removes a key from the key-value store.
    pub fn remove(&mut self, rt: &impl Runtime, key: &OwnedPath) -> Result<()> {
        self.snapshot.remove(key);
        if Storage::contains_key(rt, key)? {
            self.remove_set.insert(key.clone());
        }
        Ok(())
    }

    /// Returns the given key's corresponding entry in the transactional
    /// snapshot for in-place manipulation.
    pub fn entry<'a, 'b, V>(
        &'a mut self,
        rt: &impl Runtime,
        key: OwnedPath,
    ) -> Result<Entry<'b, V>>
    where
        V: Value + DeserializeOwned,
        'a: 'b,
    {
        self.lookup::<V>(rt, key.clone())?;

        match self.snapshot.entry(key) {
            btree_map::Entry::Vacant(inner) => Ok(Entry::Vacant(VacantEntry::new(inner))),
            btree_map::Entry::Occupied(inner) => Ok(Entry::Occupied(OccupiedEntry::new(
                &mut self.remove_set,
                inner,
            ))),
        }
    }
}

/// A view into a single entry in the transaction snapshot, which is either
/// vacant or occupied.
pub enum Entry<'a, V: 'a> {
    /// A vacant entry.
    Vacant(VacantEntry<'a, V>),

    /// An occupied entry.
    Occupied(OccupiedEntry<'a, V>),
}

impl<'a, V> Entry<'a, V> {
    pub fn or_insert_default(self) -> &'a mut V
    where
        V: Value + Default,
    {
        match self {
            Entry::Vacant(vacant_entry) => vacant_entry.insert(Default::default()),
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
        }
    }
}

/// A view into a vacant entry in the transactional snapshot.
pub struct VacantEntry<'a, V: 'a> {
    inner: btree_map::VacantEntry<'a, OwnedPath, SnapshotEntry>,
    _marker: PhantomData<V>,
}

impl<'a, V: 'a> VacantEntry<'a, V> {
    fn new(inner: btree_map::VacantEntry<'a, OwnedPath, SnapshotEntry>) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    /// Gets a reference to the key of the entry.
    pub fn key(&self) -> &OwnedPath {
        self.inner.key()
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> OwnedPath {
        self.inner.into_key()
    }

    /// Set the value of the entry using the entry's key and return a mutable
    /// reference to the value.
    pub fn insert(self, value: V) -> &'a mut V
    where
        V: Value,
    {
        self.inner
            .insert(SnapshotEntry::ephemeral::<V>(value))
            .as_mut()
    }
}

/// A view into an occupied entry in the transactional snapshot.

pub struct OccupiedEntry<'a, V: 'a> {
    remove_set: &'a mut BTreeSet<OwnedPath>,
    inner: btree_map::OccupiedEntry<'a, OwnedPath, SnapshotEntry>,
    _marker: PhantomData<V>,
}

impl<'a, V> OccupiedEntry<'a, V> {
    fn new(
        remove_set: &'a mut BTreeSet<OwnedPath>,
        inner: btree_map::OccupiedEntry<'a, OwnedPath, SnapshotEntry>,
    ) -> Self {
        Self {
            remove_set,
            inner,
            _marker: PhantomData,
        }
    }

    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &OwnedPath {
        self.inner.key()
    }

    /// Takes the key-value pair out of the snapshot, returning ownership
    /// to the caller.
    pub fn remove_entry(self) -> (OwnedPath, V)
    where
        V: Value,
    {
        let (key, entry) = self.inner.remove_entry();
        self.remove_set.insert(key.clone());
        (key, entry.into_value())
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V
    where
        V: Value,
    {
        self.inner.get().as_ref()
    }

    /// Get a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut V
    where
        V: Value,
    {
        self.inner.get_mut().as_mut()
    }

    /// Convert the entry into a mutable reference to its value.
    pub fn into_mut(self) -> &'a mut V
    where
        V: Value,
    {
        self.inner.into_mut().as_mut()
    }

    /// Sets the value of the entry and returns the entry's old value.
    pub fn insert(&mut self, value: V) -> V
    where
        V: Value,
    {
        std::mem::replace(self.get_mut(), value)
    }

    /// Take the value of the entry out of the snapshot, and return it.
    pub fn remove(self) -> V
    where
        V: Value,
    {
        self.remove_set.insert(self.key().clone());
        self.inner.remove().into_value()
    }
}
