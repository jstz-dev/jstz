use std::{
    borrow::Borrow,
    collections::{btree_map, BTreeMap, BTreeSet},
};

use serde::{de::DeserializeOwned, Serialize};
use tezos_smart_rollup_host::{
    path::{OwnedPath, RefPath},
    runtime::{Runtime, ValueType},
};

mod error {

    pub enum Error {}

    pub type Result<T> = std::result::Result<T, Error>;
}

use error::Result;

use self::transaction::Transaction;

pub trait Storage<K, V> {
    fn get(&self, key: &K) -> Result<Option<V>>;
    fn set(&mut self, key: K, val: V) -> Result<()>;
}

mod transaction {
    use super::*;

    #[derive(Debug)]
    pub struct CacheEntry<V> {
        dirty: bool,
        value: V,
    }

    impl<V> CacheEntry<V> {
        pub fn new(value: V) -> Self {
            CacheEntry {
                dirty: false,
                value,
            }
        }

        pub fn dirty(value: V) -> Self {
            CacheEntry { dirty: true, value }
        }

        pub fn get_value(&self) -> &V {
            &self.value
        }

        pub fn get_mut_value(&mut self) -> &mut V {
            self.dirty = true;
            &mut self.value
        }
    }

    pub struct Transaction<'s, K, V> {
        storage: &'s mut dyn Storage<K, V>,
        cache: BTreeMap<K, CacheEntry<V>>,
        pub begin_timestamp: u64,
    }

    impl<'s, K, V> Transaction<'s, K, V> {
        pub(crate) fn new(
            storage: &'s mut dyn Storage<K, V>,
            begin_timestamp: u64,
        ) -> Self {
            Self {
                storage,
                cache: BTreeMap::new(),
                begin_timestamp,
            }
        }

        pub(crate) fn write_set(&self) -> BTreeSet<K>
        where
            K: Ord + Clone,
        {
            self.cache
                .into_iter()
                .filter_map(|(k, entry)| if entry.dirty { Some(k.clone()) } else { None })
                .collect()
        }

        pub(crate) fn write_back(self) -> Result<()>
        where
            K: Ord,
        {
            for (k, v) in self.cache.into_iter() {
                if v.dirty {
                    self.storage.set(k, v.value)?;
                }
            }
            Ok(())
        }

        fn get(&mut self, key: &K) -> Result<Option<&V>>
        where
            K: Ord + Clone,
        {
            let entry = self.cache.entry(key.clone());

            match entry {
                btree_map::Entry::Vacant(entry) => {
                    if let Some(value) = self.storage.get(key)? {
                        let cache_entry = entry.insert(CacheEntry::new(value));

                        return Ok(Some(&cache_entry.value));
                    }

                    Ok(None)
                }
                btree_map::Entry::Occupied(entry) => Ok(Some(&entry.into_mut().value)),
            }
        }

        fn insert(&mut self, key: K, value: V) -> Result<()>
        where
            K: Ord,
        {
            self.cache.insert(key, CacheEntry::dirty(value));
            Ok(())
        }
    }

    #[derive(Debug)]
    pub enum Entry<'a, K: Ord + 'a, V: 'a> {
        Vacant(VacantEntry<'a, K, V>),
        Occupied(OccupiedEntry<'a, K, V>),
    }

    #[derive(Debug)]
    pub struct VacantEntry<'a, K: Ord + 'a, V: 'a>(
        btree_map::VacantEntry<'a, K, CacheEntry<V>>,
    );

    impl<'a, K: Ord, V> VacantEntry<'a, K, V> {
        pub fn key(&self) -> &K {
            self.0.key()
        }

        pub fn into_key(self) -> K {
            self.0.into_key()
        }

        pub fn insert(self, value: V) -> &'a mut V {
            &mut self.0.insert(CacheEntry::dirty(value)).value
        }
    }

    #[derive(Debug)]
    pub struct OccupiedEntry<'a, K: Ord + 'a, V: 'a>(
        btree_map::OccupiedEntry<'a, K, CacheEntry<V>>,
    );

    impl<'a, K: Ord, V> OccupiedEntry<'a, K, V> {
        pub fn key(&self) -> &K {
            self.0.key()
        }

        pub fn remove_entry(self) -> (K, V) {
            // TODO: We need to insert a tombstone
            todo!()
        }

        pub fn get(&self) -> &V {
            self.0.get().get_value()
        }

        pub fn get_mut(&mut self) -> &mut V {
            self.0.get_mut().get_mut_value()
        }

        pub fn into_mut(self) -> &'a mut V {
            self.0.into_mut().get_mut_value()
        }

        pub fn insert(&mut self, value: V) -> V {
            std::mem::replace(self.get_mut(), value)
        }

        pub fn remove(self) -> V {
            todo!()
        }
    }
}

use transaction::*;

const MAX_TX_COUNT: usize = 16;

pub struct Database<'s, K, V> {
    storage: &'s mut dyn Storage<K, V>,
    clock: u64, // logical timestamps
    write_sets: BTreeMap<u64, BTreeSet<K>>,
}

impl<'s, K, V> Database<'s, K, V> {
    pub fn new(storage: &'s mut dyn Storage<K, V>) -> Self {
        Self {
            storage,
            clock: 0,
            write_sets: BTreeMap::new(),
        }
    }

    pub fn begin_transaction<'a>(&'s mut self) -> Transaction<'a, K, V>
    where
        's: 'a,
    {
        Transaction::new(self.storage, self.clock)
    }

    pub fn commit_transaction(&mut self, tx: Transaction<'_, K, V>) -> Result<()>
    where
        K: Ord + Clone,
    {
        let finish_timestamp = self.clock;
        let write_set = tx.write_set();

        // Validate
        for ts in tx.begin_timestamp + 1..finish_timestamp + 1 {
            match self.write_sets.get(&ts) {
                Some(other_write_set) => {
                    if write_set.intersection(other_write_set).count() > 0 {
                        return Ok(());
                    }
                }
                None => return Ok(()),
            }
        }

        // Write back
        tx.write_back();

        self.clock += 1;
        self.write_sets.insert(self.clock, write_set);
        if self.clock - MAX_TX_COUNT as u64 > 0 {
            self.write_sets.remove(&(self.clock - MAX_TX_COUNT as u64));
        }

        Ok(())
    }
}
