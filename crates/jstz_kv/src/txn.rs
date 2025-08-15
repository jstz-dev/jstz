//! Transaction management for the KV store.
//!
//! This module implements ACID transaction support for the key-value store using
//! strict two-phase locking (2PL) with wound-wait deadlock prevention.

use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
    mem,
    ops::{Deref, DerefMut},
};

use tokio_util::sync::CancellationToken;

use crate::{
    db::{Db, EntryValue},
    wound_wait_mutex::{ArcWoundWaitMutexGuard, LockError},
};

/// A guard table entry value that holds both the current guarded value and the original committed value.
#[derive(Debug)]
struct GuardTableEntryValue<Id: Ord, V> {
    /// The guarded value of the entry, protected by a wound-wait mutex in the [`MemTable`].
    guard: ArcWoundWaitMutexGuard<Id, EntryValue<V>>,
    /// The original committed value before this transaction began modifying it.
    /// This is `None` if the entry was first accessed in this transaction and had no prior modifications.
    committed_value: Option<EntryValue<V>>,
}

impl<Id: Ord, V> Clone for GuardTableEntryValue<Id, V>
where
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            guard: self.guard.clone(),
            committed_value: self.committed_value.clone(),
        }
    }
}

/// Manages all guard entries for a transaction.
#[derive(Debug)]
struct GuardTable<Id: Ord, K, V> {
    guards: HashMap<K, GuardTableEntryValue<Id, V>>,
}

impl<Id: Ord, K, V> Default for GuardTable<Id, K, V> {
    fn default() -> Self {
        Self {
            guards: HashMap::new(),
        }
    }
}

#[derive(Debug)]
enum TxnState {
    Active,
    Committed,
    Aborted,
}

/// A read-write transaction on the database.
#[derive(Debug)]
pub struct Txn<Id: Ord, K, V> {
    /// Reference to the database this transaction operates on.
    db: Db<Id, K, V>,
    /// Unique identifier for this transaction, used for deadlock prevention.
    id: Id,
    /// Table of all guards (locks) held by this transaction.
    guard_table: GuardTable<Id, K, V>,
    /// Token used to cancel long-running operations in this transaction.
    cancellation_token: CancellationToken,
    /// The transaction's state
    state: TxnState,
}

/// Error type for transaction operations.
#[derive(Debug, PartialEq, Eq)]
pub enum TxnError {
    /// Failed to acquire a lock on a key.
    LockAcquisitionFailed(LockError),
}

impl std::fmt::Display for TxnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxnError::LockAcquisitionFailed(lock_error) => {
                write!(f, "Failed to acquire lock: {lock_error}")
            }
        }
    }
}

impl std::error::Error for TxnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TxnError::LockAcquisitionFailed(lock_error) => Some(lock_error),
        }
    }
}

impl From<LockError> for TxnError {
    fn from(error: LockError) -> Self {
        TxnError::LockAcquisitionFailed(error)
    }
}

/// A wrapper around [`ArcWoundWaitMutexGuard`] that prevents write access.
pub struct TxnGuard<Id: Ord, T>(ArcWoundWaitMutexGuard<Id, T>);

impl<Id: Ord, T> Deref for TxnGuard<Id, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<Id: Ord, T> Clone for TxnGuard<Id, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Id: Ord, K, V> Txn<Id, K, V> {
    /// Creates a new transaction with the given database, ID, and cancellation token.
    pub fn new(db: Db<Id, K, V>, id: Id, cancellation_token: CancellationToken) -> Self {
        Self {
            state: TxnState::Active,
            db,
            id,
            guard_table: GuardTable::default(),
            cancellation_token,
        }
    }

    #[inline]
    fn is_active(&self) -> bool {
        matches!(self.state, TxnState::Active)
    }

    /// Returns the unique identifier of this transaction.
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Returns a reference to the database this transaction operates on.
    pub fn db(&self) -> &Db<Id, K, V> {
        &self.db
    }

    /// Returns the cancellation token for this transaction.
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Gets or creates a mutable reference to a guard entry for the specified key.
    ///
    /// If the key is already locked by this transaction, returns the existing guard entry.
    /// Otherwise, acquires a new lock on the key and creates a new guard entry.
    async fn get_guard_entry_mut(
        &mut self,
        key: K,
    ) -> Result<&mut GuardTableEntryValue<Id, V>, TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        assert!(self.is_active());

        match self.guard_table.guards.entry(key) {
            Entry::Occupied(occupied_entry) => Ok(occupied_entry.into_mut()),
            Entry::Vacant(vacant_entry) => {
                let guard = self
                    .db
                    .mem_table
                    .lock(
                        vacant_entry.key().clone(),
                        self.id.clone(),
                        self.cancellation_token.clone(),
                    )
                    .await?;

                let guard_table_entry = GuardTableEntryValue {
                    guard,
                    committed_value: None,
                };

                Ok(vacant_entry.insert(guard_table_entry))
            }
        }
    }

    /// A function to inspect the committed value of a key. Used in testing
    #[cfg(test)]
    async fn committed_value(
        &mut self,
        key: &K,
    ) -> Result<Option<&EntryValue<V>>, TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        let guard_entry = self.get_guard_entry_mut(key.clone()).await?;

        Ok(guard_entry.committed_value.as_ref())
    }

    /// Looks for the key in the transction.
    ///
    /// Returns `None` if the key does not exist. The value is wrapped in [`TxnGuard`]
    /// which allows us to dereference the value.
    pub async fn get(&mut self, key: &K) -> Result<TxnGuard<Id, Option<V>>, TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        let guard_entry = self.get_guard_entry_mut(key.clone()).await?;

        Ok(TxnGuard(guard_entry.guard.clone()))
    }

    async fn modify(&mut self, key: K, value: EntryValue<V>) -> Result<(), TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        let guard_entry = self.get_guard_entry_mut(key).await?;

        let maybe_committed_value = mem::replace(guard_entry.guard.deref_mut(), value);
        if guard_entry.committed_value.is_none() {
            // `maybe_committed_value` is known to be a committed value.
            guard_entry.committed_value = Some(maybe_committed_value);
        }

        Ok(())
    }

    /// Inserts a key-value pair into the transaction.
    ///
    /// The value is immediately visible within this transaction but will only become
    /// persistent after the transaction is committed. If the key already exists,
    /// its value is replaced.
    pub async fn insert(&mut self, key: K, value: V) -> Result<(), TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        self.modify(key, Some(value)).await
    }

    /// Removes a key from the transaction.
    ///
    /// This operation creates a tombstone for the key, marking it as deleted within
    /// this transaction. The deletion will only become persistent after the transaction
    /// is committed.
    pub async fn remove(&mut self, key: K) -> Result<(), TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        self.modify(key, None).await
    }

    /// Returns `true` if the transaction contains the specified key.
    ///
    /// This method checks whether the key exists within the current transaction's view,
    /// which includes both committed data and any modifications made within this transaction.
    pub async fn contains_key(&mut self, key: &K) -> Result<bool, TxnError>
    where
        K: Eq + Hash + Clone,
        Id: Clone,
    {
        let guard_entry = self.get_guard_entry_mut(key.clone()).await?;

        Ok(guard_entry.guard.is_some())
    }

    /// Commits all changes made in this transaction.
    ///
    /// This operation makes all modifications within the transaction permanent and
    /// releases all acquired locks. Once committed, the transaction is consumed and
    /// cannot be used for further operations.
    pub fn commit(mut self) {
        assert!(self.is_active());

        self.state = TxnState::Committed;

        // All the changes are already applied to the `db.mem_table` through the guards.
        // When we drop the transaction, all guards will be automatically released.
        // The guards already contain the modified values, so the commit is essentially
        // a noop.

        // The guards are automatically dropped when the transaction is dropped,
        // which happens at the end of this function.
    }

    /// Rolls back all changes made in this transaction.
    ///
    /// This operation reverts all modifications made within the transaction to their
    /// original committed state and releases all acquired locks. Once rolled back,
    /// the transaction is consumed and cannot be used for further operations.
    pub fn rollback(mut self) {
        self.rollback_inner();
    }

    fn rollback_inner(&mut self) {
        assert!(self.is_active());

        // Revert all changes by restoring the committed values
        for (_, guard_entry) in self.guard_table.guards.iter_mut() {
            if let Some(committed_value) = guard_entry.committed_value.take() {
                // Restore the original value
                *guard_entry.guard.deref_mut() = committed_value;
            }
        }

        // Guards are automatically dropped when the guard table is cleared.
        self.guard_table.guards.clear();
        self.state = TxnState::Aborted;
    }
}

impl<Id: Ord, K, V> Drop for Txn<Id, K, V> {
    fn drop(&mut self) {
        if self.is_active() {
            self.rollback_inner();
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use futures::future::pending;
    use tokio::{select, sync::oneshot};
    use tokio_util::sync::CancellationToken;

    type TestDb = Db<i32, &'static str, &'static str>;
    type TestTxn = Txn<i32, &'static str, &'static str>;

    fn new_txn(db: &TestDb, id: i32) -> TestTxn {
        Txn::new(db.clone(), id, CancellationToken::new())
    }

    async fn lock_db(
        db: &TestDb,
        key: &&'static str,
    ) -> ArcWoundWaitMutexGuard<i32, EntryValue<&'static str>> {
        db.mem_table
            .lock(key, 999, CancellationToken::new())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_new_txn() {
        let db = Db::new();
        let txn = new_txn(&db, 1);

        assert_eq!(*txn.id(), 1);
        assert!(!txn.cancellation_token().is_cancelled());
    }

    #[tokio::test]
    async fn test_get() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        let guard = txn.get(&"key1").await.unwrap();
        assert!(guard.is_none());

        assert!(txn.committed_value(&"key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        txn.insert("key1", "value1").await.unwrap();

        // The `.insert` method should have updated the value, storing the committed value
        // in the guard table.
        assert!(matches!(
            txn.committed_value(&"key1").await.unwrap(),
            Some(None)
        ));

        let guard = txn.get(&"key1").await.unwrap();
        assert_eq!(guard.unwrap(), "value1");
    }

    #[tokio::test]
    async fn test_contains_key() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        assert!(!txn.contains_key(&"key1").await.unwrap())
    }

    #[tokio::test]
    async fn test_insert_and_contains_key() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        assert!(!txn.contains_key(&"key1").await.unwrap());

        txn.insert("key1", "value1").await.unwrap();
        assert!(txn.contains_key(&"key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_remove() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        txn.insert("key1", "value1").await.unwrap();
        assert!(txn.contains_key(&"key1").await.unwrap());

        txn.remove("key1").await.unwrap();
        assert!(!txn.contains_key(&"key1").await.unwrap());

        // The `.remove` method should have updated the value, storing the committed value
        // in the guard table.
        assert!(matches!(
            txn.committed_value(&"key1").await.unwrap(),
            Some(None)
        ));

        let guard = txn.get(&"key1").await.unwrap();
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_commit() {
        let db = Db::new();
        let mut txn = new_txn(&db, 1);

        txn.insert("key1", "value1").await.unwrap();
        txn.commit();

        // After commit, verify data is persisted by acquiring locks on `db`
        let guard = lock_db(&db, &"key1").await;
        assert_eq!(guard.unwrap(), "value1");
    }

    #[tokio::test]
    async fn test_rollback() {
        let db = Db::new();

        // Insert some data and commit
        let mut txn1 = new_txn(&db, 1);
        txn1.insert("key1", "original").await.unwrap();
        txn1.commit();

        // Modify and rollback
        let mut txn2 = new_txn(&db, 1);
        txn2.insert("key1", "modified").await.unwrap();
        txn2.rollback();

        // Original value is restored
        let guard = lock_db(&db, &"key1").await;
        assert_eq!(guard.unwrap(), "original");
    }

    #[tokio::test]
    async fn test_drop_rollback() {
        let db = Db::new();

        let mut txn1 = new_txn(&db, 1);
        txn1.insert("key1", "original").await.unwrap();
        txn1.commit();

        {
            let mut txn2 = new_txn(&db, 2);
            txn2.insert("key1", "modified").await.unwrap();
            // txn2 drops here without commit
        }

        let guard = lock_db(&db, &"key1").await;
        assert_eq!(guard.unwrap(), "original");
    }

    #[tokio::test]
    async fn test_wound_wait() {
        let db = Db::new();

        let mut txn1 = new_txn(&db, 1);
        let mut txn2 = new_txn(&db, 2);

        // Use a channel to control execution order
        let (txn2_locked_tx, txn2_locked_rx) = oneshot::channel::<()>();

        let cancel2 = txn2.cancellation_token().clone();

        // We acquire the lock in the younger txn
        let task2 = tokio::spawn(async move {
            let cancel2 = txn2.cancellation_token().clone();
            select! {
                _ = cancel2.cancelled() => {},
                _ = async move {
                    // txn2 gets a lock on "key"
                    txn2.insert("key", "value2").await.unwrap();

                    // Signal that txn2 has acquired the lock
                    txn2_locked_tx.send(()).unwrap();

                    // Wait forever
                    pending().await
                } => {}
            }
        });

        // txn1 tries to acquire the same lock (older transaction)
        // txn1 will wound the txn2's lock in task2.
        let task1 = tokio::spawn(async move {
            let () = txn2_locked_rx.await.unwrap();

            // Now txn1 tries to get the same "key" - this should wound txn2
            txn1.insert("key", "value1").await.unwrap();

            // Verify that txn1's cancellation token is not cancelled
            assert!(!txn1.cancellation_token().is_cancelled());

            // Verify that txn2's cancellation token is cancelled
            assert!(cancel2.is_cancelled());

            // txn1 can now commit successfully
            txn1.commit();
        });

        let (result1, result2) = tokio::join!(task1, task2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Check the committed value is txn1's value
        let guard = lock_db(&db, &"key").await;
        assert_eq!(guard.unwrap(), "value1");
    }
}
