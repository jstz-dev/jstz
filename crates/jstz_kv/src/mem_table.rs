//! A simple global mem table that manages an in-memory copy of the KV store
//! with wound-wait mutexes for different keys. Granularity is at the key level.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
};

use tokio_util::sync::CancellationToken;

use crate::wound_wait_mutex::{ArcWoundWaitMutexGuard, LockError, WoundWaitMutex};

/// A in-memory table that manages wound-wait mutexes for individual keys,
/// protecting the corresponding values in the KV store.
#[derive(Debug)]
pub struct MemTable<Id: Ord, K, V> {
    mutexes: Mutex<HashMap<K, Arc<WoundWaitMutex<Id, Option<V>>>>>,
}

impl<Id: Ord, K, V> Default for MemTable<Id, K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Ord, K, V> MemTable<Id, K, V> {
    /// Creates a new empty mem table.
    pub fn new() -> Self {
        Self {
            mutexes: Mutex::new(HashMap::new()),
        }
    }

    /// Gets the mutex for the given key.
    ///
    /// If no mutex exists, creates a new one using `V::default()`.
    fn get_or_insert_mutex(&self, key: K) -> Arc<WoundWaitMutex<Id, Option<V>>>
    where
        K: Hash + Eq,
    {
        // TODO(https://linear.app/tezos/issue/JSTZ-886):
        // Instead of using `Option<V>::default()`, we should provide a way to fetch
        // the value from persistent storage.

        // SAFETY: We exclusively own the mutex, so we know it cannot be poisoned.
        let mut mutexes = self.mutexes.lock().expect("Mutex cannot be poisoned");

        mutexes.entry(key).or_default().clone()
    }

    /// Acquire a lock on the specified key.
    pub async fn lock(
        &self,
        key: K,
        transaction_id: Id,
        cancellation_token: CancellationToken,
    ) -> Result<ArcWoundWaitMutexGuard<Id, Option<V>>, LockError>
    where
        K: Hash + Eq,
        Id: Clone,
    {
        let mutex = self.get_or_insert_mutex(key);
        mutex.lock_arc(transaction_id, cancellation_token).await
    }

    /// Removes unused keys from the mem table.
    ///
    /// This method removes any mutex that has only one reference (the one held
    /// by the lock table itself), indicating that no other part of the system
    /// is currently using it.
    ///
    /// This is useful for preventing memory leaks in long-running applications
    /// where many different keys might be accessed over time.
    ///
    /// # Returns
    ///
    /// The number of mutexes that were cleaned up.
    pub fn gc(&self) -> usize {
        // SAFETY: We exclusively own the mutex, so we know it cannot be poisoned.
        let mut mutexes = self.mutexes.lock().expect("Mutex cannot be poisoned");
        let initial_count = mutexes.len();

        mutexes.retain(|_, mutex| Arc::strong_count(mutex) > 1);

        initial_count - mutexes.len()
    }

    /// Returns the number of mutexes currently in the lock table.
    /// Used for testing purposes to verify the number of mutexes in the lock table.
    #[cfg(test)]
    fn mutex_count(&self) -> usize {
        // SAFETY: We exclusively own the mutex, so we know it cannot be poisoned.
        let mutexes = self.mutexes.lock().expect("Mutex cannot be poisoned");
        mutexes.len()
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn new_mem_table<V>() -> MemTable<u64, &'static str, V> {
        MemTable::new()
    }

    #[tokio::test]
    async fn test_lock_key() {
        let mem_table = new_mem_table::<()>();

        let id0 = 0;
        let cancel0 = CancellationToken::new();

        let _guard = mem_table.lock("key1", id0, cancel0).await.unwrap();

        assert_eq!(mem_table.mutex_count(), 1);
    }

    #[tokio::test]
    async fn test_mem_multiple_keys() {
        let mem_table = new_mem_table::<i32>();

        let id0 = 0;
        let cancel0 = CancellationToken::new();

        let guard1 = mem_table.lock("key1", id0, cancel0.clone()).await.unwrap();
        let guard2 = mem_table.lock("key2", id0, cancel0.clone()).await.unwrap();

        // Drop the mutex on key1
        drop(guard1);

        // Acquire another lock on key1 (this should not create a new mutex)
        let guard3 = mem_table.lock("key1", id0, cancel0).await.unwrap();

        assert_eq!(mem_table.mutex_count(), 2);
        drop(guard3);
        drop(guard2);
    }

    #[tokio::test]
    async fn test_gc() {
        let mem_table = new_mem_table::<()>();

        let id0 = 0;
        let cancel0 = CancellationToken::new();

        // Create some mutexes
        {
            let _guard1 = mem_table.lock("key1", id0, cancel0.clone()).await.unwrap();
            let _guard2 = mem_table.lock("key2", id0, cancel0.clone()).await.unwrap();
            assert_eq!(mem_table.mutex_count(), 2);
        } // Guards are dropped here

        // Garbage collect unused mutexes, should remove both
        let gcd = mem_table.gc();
        assert_eq!(gcd, 2);
    }
}
