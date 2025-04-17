use std::{
    collections::{btree_map, BTreeMap, BTreeSet},
    marker::PhantomData,
    mem,
};

use derive_more::{Deref, DerefMut};

use tezos_smart_rollup_host::{path::OwnedPath, runtime::Runtime};

use super::{
    outbox::{
        flush, OutboxError, OutboxMessage, PersistentOutboxQueue, SnapshotOutboxQueue,
    },
    value::{BoxedValue, Value},
    Storage,
};
use crate::error::{KvError, Result};

/// A transaction is a 'lazy' snapshot of the persistent key-value store from
/// the point in time when the transaction began. Modifications to new or old
/// values within the transaction remain isolated from any concurrent
/// transactions.
///
/// Reads are cached for each transaction, optimizing repeated accesses to the
/// same key. Writes are buffered in using an in-memory representation until the
/// root transaction is successfully committed, at which point the buffer is flushed
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
///
/// NOTE: Current implementation does NOT support concurrent transactions

/// A key is a path in durable storage
pub type Key = OwnedPath;

// A lookup map is a history of edits of a given key in order of least-recent to most-recent
// This allows O(log n) lookups, and O(log n) commits / rollbacks (amortized by # of inserts / removals).
#[derive(Debug, Default, Deref, DerefMut)]
struct LookupMap(BTreeMap<Key, Vec<usize>>);

#[derive(Debug, Default)]
pub struct Transaction {
    // A stack of transactional snapshots
    stack: Vec<Snapshot>,
    lookup_map: LookupMap,
    persistent_outbox: PersistentOutboxQueue,
    snapshot_outbox_len: u32,
}

#[derive(Debug, Clone, Deref, DerefMut)]
struct SnapshotValue(BoxedValue);

impl SnapshotValue {
    pub fn new(value: impl Value) -> Self {
        Self(BoxedValue::new(value))
    }

    pub fn as_ref<V: Value>(&self) -> Result<&V> {
        Ok(self
            .as_any()
            .downcast_ref()
            .ok_or(KvError::DowncastFailed)?)
    }

    pub fn as_mut<V: Value>(&mut self) -> Result<&mut V> {
        Ok(self
            .as_any_mut()
            .downcast_mut()
            .ok_or(KvError::DowncastFailed)?)
    }

    pub fn into_value<V: Value>(self) -> Result<V> {
        let value = self.0.downcast().map_err(|_| KvError::DowncastFailed)?;
        *value
    }
}

#[derive(Debug, Default)]
struct Snapshot {
    // INVARIANT: Set of keys in the edits are disjoint
    // A map of 'insert' edits to be applied
    insert_edits: BTreeMap<Key, SnapshotValue>,
    // A set of 'remove' edits to be applied
    remove_edits: BTreeSet<Key>,
    outbox_queue: SnapshotOutboxQueue,
}

impl Snapshot {
    pub fn insert(&mut self, key: Key, value: SnapshotValue) {
        self.remove_edits.remove(&key);
        self.insert_edits.insert(key, value);
    }

    pub fn remove(&mut self, key: Key) {
        self.insert_edits.remove(&key);
        self.remove_edits.insert(key);
    }

    pub fn lookup(&self, key: &Key) -> Option<&SnapshotValue> {
        if self.remove_edits.contains(key) {
            return None;
        }

        self.insert_edits.get(key)
    }

    pub fn lookup_mut(&mut self, key: &Key) -> Option<&mut SnapshotValue> {
        if self.remove_edits.contains(key) {
            return None;
        }

        self.insert_edits.get_mut(key)
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        self.insert_edits.contains_key(key) && !self.remove_edits.contains(key)
    }

    pub fn outbox_queue_mut(&mut self) -> &mut SnapshotOutboxQueue {
        &mut self.outbox_queue
    }
}

impl LookupMap {
    fn update(&mut self, key: Key, idx: usize) {
        let key_history = self.entry(key).or_default();

        match key_history.last() {
            Some(&last_idx) if last_idx == idx => {
                // The key was already looked up in the current context
            }
            _ => {
                key_history.push(idx);
            }
        }
    }

    fn rollback(&mut self, key: &Key) -> Result<()> {
        let is_history_empty = {
            let history = self.get_mut(key).ok_or(KvError::ExpectedLookupMapEntry)?;

            history.pop();
            history.is_empty()
        };

        if is_history_empty {
            self.remove(key);
        }

        Ok(())
    }
}

impl Transaction {
    fn current_snapshot_idx(&self) -> usize {
        self.stack.len().saturating_sub(1)
    }

    fn update_lookup_map(&mut self, key: Key) {
        self.lookup_map.update(key, self.current_snapshot_idx())
    }

    /// Return the current snapshot
    fn current_snapshot(&mut self) -> Result<&mut Snapshot> {
        Ok(self
            .stack
            .last_mut()
            .ok_or(KvError::TransactionStackEmpty)?)
    }

    /// Insert a key-value pair into the current snapshot (as a 'insert' edit)
    fn current_snapshot_insert(&mut self, key: Key, value: SnapshotValue) -> Result<()> {
        self.update_lookup_map(key.clone());
        self.current_snapshot()?.insert(key, value);
        Ok(())
    }

    /// Lookup a key in the current snapshot
    fn current_snapshot_lookup(&mut self, key: &Key) -> Result<Option<&SnapshotValue>> {
        Ok(self.current_snapshot()?.lookup(key))
    }

    /// Lookup a key in the current snapshot
    fn current_snapshot_lookup_mut(
        &mut self,
        key: &Key,
    ) -> Result<Option<&mut SnapshotValue>> {
        Ok(self.current_snapshot()?.lookup_mut(key))
    }

    /// Remove a key from the current snapshot (as a 'remove' edit)
    fn current_snapshot_remove(&mut self, key: Key) -> Result<()> {
        self.update_lookup_map(key.clone());
        self.current_snapshot()?.remove(key);
        Ok(())
    }

    fn lookup<V: Value>(
        &mut self,
        rt: &impl Runtime,
        key: Key,
    ) -> Result<Option<&SnapshotValue>> {
        if let Some(&snapshot_idx) =
            self.lookup_map.get(&key).and_then(|history| history.last())
        {
            let snapshot = &self.stack[snapshot_idx];

            return Ok(snapshot.lookup(&key));
        }

        if let Some(value) = Storage::get::<V>(rt, &key)? {
            // TODO: This clone is probably not necessary
            self.current_snapshot_insert(key.clone(), SnapshotValue::new(value))?;

            self.current_snapshot_lookup(&key)
        } else {
            Ok(None)
        }
    }

    fn lookup_mut<V: Value>(
        &mut self,
        rt: &impl Runtime,
        key: Key,
    ) -> Result<Option<&mut SnapshotValue>> {
        if let Some(&snapshot_idx) =
            self.lookup_map.get(&key).and_then(|history| history.last())
        {
            let snapshot = &self.stack[snapshot_idx];

            if let Some(value) = snapshot.lookup(&key) {
                self.current_snapshot_insert(key.clone(), value.clone())?;
                self.current_snapshot_lookup_mut(&key)
            } else {
                Ok(None)
            }
        } else if let Some(value) = Storage::get::<V>(rt, &key)? {
            self.current_snapshot_insert(key.clone(), SnapshotValue::new(value))?;
            self.current_snapshot_lookup_mut(&key)
        } else {
            Ok(None)
        }
    }

    /// Returns a reference to the value corresponding to the key in the
    /// key-value store if it exists.
    pub fn get<V: Value>(&mut self, rt: &impl Runtime, key: Key) -> Result<Option<&V>>
where {
        self.lookup::<V>(rt, key)
            .map(|entry_opt| entry_opt.map(|entry| entry.as_ref()).transpose())?
    }

    /// Returns a mutable reference to the value corresponding to the key in the
    /// key-value store if it exists.
    pub fn get_mut<V: Value>(
        &mut self,
        rt: &impl Runtime,
        key: Key,
    ) -> Result<Option<&mut V>> {
        self.lookup_mut::<V>(rt, key)
            .map(|entry_opt| entry_opt.map(|entry| entry.as_mut()).transpose())?
    }

    /// Returns `true` if the key-value store contains a key-value pair for the
    /// specified key.
    pub fn contains_key(&self, rt: &impl Runtime, key: &Key) -> Result<bool> {
        if let Some(&context_idx) =
            self.lookup_map.get(key).and_then(|history| history.last())
        {
            let context = &self.stack[context_idx];

            return Ok(context.contains_key(key));
        }

        Storage::contains_key(rt, key)
    }

    /// Insert a key-value pair into the key-value store.
    pub fn insert<V: Value>(&mut self, key: Key, value: V) -> Result<()> {
        self.current_snapshot_insert(key, SnapshotValue::new(value))
    }

    /// Removes a key from the key-value store.
    pub fn remove(&mut self, key: Key) -> Result<()> {
        self.current_snapshot_remove(key)
    }

    /// Returns the given key's corresponding entry in the transactional
    /// snapshot for in-place manipulation.
    pub fn entry<'a, 'b, V>(
        &'a mut self,
        rt: &impl Runtime,
        key: Key,
    ) -> Result<Entry<'b, V>>
    where
        V: Value,
        'a: 'b,
    {
        // A mutable lookup ensures the key is in the current snapshot
        self.lookup_mut::<V>(rt, key.clone())?;

        let current_snapshot_idx = self.current_snapshot_idx();
        // self.current_snapshot() inlined to avoid lifetime issue
        let current_snapshot = self
            .stack
            .last_mut()
            .ok_or(KvError::TransactionStackEmpty)?;

        match current_snapshot.insert_edits.entry(key) {
            btree_map::Entry::Vacant(inner) => Ok(Entry::vacant(
                inner,
                &mut self.lookup_map,
                current_snapshot_idx,
            )),
            btree_map::Entry::Occupied(inner) => {
                Ok(Entry::occupied(inner, &mut current_snapshot.remove_edits))
            }
        }
    }

    pub fn queue_outbox_message(
        &mut self,
        rt: &mut impl Runtime,
        message: OutboxMessage,
    ) -> Result<()> {
        if self.persistent_outbox.len(rt)? + self.snapshot_outbox_len + 1
            > self.persistent_outbox.max(rt)?
        {
            Err(OutboxError::OutboxQueueFull)?;
        }
        let current_outbox_queue = self.current_snapshot()?.outbox_queue_mut();
        current_outbox_queue.queue_message(message);
        self.snapshot_outbox_len += 1;
        Ok(())
    }

    /// Begin a transaction.
    pub fn begin(&mut self) {
        self.stack.push(Snapshot::default())
    }

    /// Commit a transaction.
    pub fn commit(&mut self, rt: &mut impl Runtime) -> Result<()> {
        let curr_ctxt = self.stack.pop().ok_or(KvError::TransactionStackEmpty)?;

        // Following the `.pop`, `prev_idx` is the index of prev_idx (if it exists)
        let prev_idx = self.current_snapshot_idx();

        if let Some(prev_ctxt) = self.stack.last_mut() {
            // TODO: These clones are probably uncessary since the entry of btree will always be occupied.
            for key in curr_ctxt.remove_edits {
                self.lookup_map.rollback(&key)?;
                self.lookup_map.update(key.clone(), prev_idx);
                prev_ctxt.remove(key);
            }

            for (key, value) in curr_ctxt.insert_edits {
                self.lookup_map.rollback(&key)?;
                self.lookup_map.update(key.clone(), prev_idx);
                prev_ctxt.insert(key, value);
            }

            prev_ctxt.outbox_queue.extend(curr_ctxt.outbox_queue);
        } else {
            for key in &curr_ctxt.remove_edits {
                Storage::remove(rt, key)?
            }

            for (key, value) in curr_ctxt.insert_edits {
                Storage::insert(rt, &key, value.0.as_ref())?
            }

            flush(rt, &mut self.persistent_outbox, curr_ctxt.outbox_queue)?;
            self.snapshot_outbox_len = 0;

            // Update lookup map
            self.lookup_map.clear()
        }

        Ok(())
    }

    /// Rollback a transaction.
    pub fn rollback(&mut self) -> Result<()> {
        let curr_ctxt = self.stack.pop().ok_or(KvError::TransactionStackEmpty)?;

        // SAFETY: The set of keys between removal edits and insertion edits are disjoint, meaning no
        // `lookup_map` entries will be rolledback more than once
        for key in &curr_ctxt.remove_edits {
            self.lookup_map.rollback(key)?;
        }

        for key in curr_ctxt.insert_edits.keys() {
            self.lookup_map.rollback(key)?
        }

        Ok(())
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
    fn vacant(
        inner: btree_map::VacantEntry<'a, Key, SnapshotValue>,
        lookup_map: &'a mut LookupMap,
        snapshot_idx: usize,
    ) -> Self {
        Entry::Vacant(VacantEntry {
            inner,
            lookup_map,
            snapshot_idx,
            _marker: PhantomData,
        })
    }

    fn occupied(
        inner: btree_map::OccupiedEntry<'a, Key, SnapshotValue>,
        remove_edits: &'a mut BTreeSet<Key>,
    ) -> Self {
        Entry::Occupied(OccupiedEntry {
            inner,
            remove_edits,
            _marker: PhantomData,
        })
    }

    pub fn or_insert_default(self) -> &'a mut V
    where
        V: Value + Default,
    {
        self.or_insert_with(|| V::default())
    }

    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        V: Value,
        F: FnOnce() -> V,
    {
        match self {
            Entry::Vacant(vacant_entry) => vacant_entry.insert(default()),
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
        }
    }
}

/// A view into a vacant entry in the transactional snapshot.
pub struct VacantEntry<'a, V: 'a> {
    inner: btree_map::VacantEntry<'a, Key, SnapshotValue>,
    // Reference to lookup map (if we insert into the vacant entry)
    lookup_map: &'a mut LookupMap,
    snapshot_idx: usize,
    _marker: PhantomData<V>,
}

impl<'a, V: 'a> VacantEntry<'a, V> {
    /// Gets a reference to the key of the entry.
    pub fn key(&self) -> &Key {
        self.inner.key()
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> Key {
        self.inner.into_key()
    }

    /// Set the value of the entry using the entry's key and return a mutable
    /// reference to the value.
    pub fn insert(self, value: V) -> &'a mut V
    where
        V: Value,
    {
        self.lookup_map
            .update(self.key().clone(), self.snapshot_idx);
        self.inner
            .insert(SnapshotValue::new(value))
            .as_mut()
            .expect("Invalid type id invariant")
    }
}

/// A view into an occupied entry in the transactional snapshot.

pub struct OccupiedEntry<'a, V: 'a> {
    inner: btree_map::OccupiedEntry<'a, Key, SnapshotValue>,
    // Reference to the set of keys to be removed from the current snapshot
    remove_edits: &'a mut BTreeSet<Key>,
    _marker: PhantomData<V>,
}

impl<'a, V> OccupiedEntry<'a, V> {
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &Key {
        self.inner.key()
    }

    /// Takes the key-value pair out of the snapshot, returning ownership
    /// to the caller.
    pub fn remove_entry(self) -> (Key, V)
    where
        V: Value,
    {
        let (key, entry) = self.inner.remove_entry();
        self.remove_edits.insert(key.clone());
        (key, entry.into_value().expect("Invalid type id invariant"))
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V
    where
        V: Value,
    {
        self.inner
            .get()
            .as_ref()
            .expect("Invalid type id invariant")
    }

    /// Get a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut V
    where
        V: Value,
    {
        self.inner
            .get_mut()
            .as_mut()
            .expect("Invalid type id invariant")
    }

    /// Convert the entry into a mutable reference to its value.
    pub fn into_mut(self) -> &'a mut V
    where
        V: Value,
    {
        self.inner
            .into_mut()
            .as_mut()
            .expect("Invalid type id invariant")
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
        self.remove_edits.insert(self.key().clone());
        self.inner
            .remove()
            .into_value()
            .expect("Invalid type id invariant")
    }
}

#[derive(Debug, Deref, DerefMut)]
pub struct JsTransaction {
    inner: &'static mut Transaction,
}

impl JsTransaction {
    pub fn new(tx: &mut Transaction) -> Self {
        // SAFETY
        // From the pov of the `JsTransaction` struct, it is permitted to cast
        // the `tx` reference to `'static` since the lifetime of `JsTransaction`
        // is always shorter than the lifetime of `tx`
        let rt: &'static mut Transaction = unsafe { mem::transmute(tx) };

        Self { inner: rt }
    }
}

#[cfg(test)]
mod test {
    use bincode::{Decode, Encode};
    use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
    use serde::{Deserialize, Serialize};
    use tezos_data_encoding::nom::NomReader;
    use tezos_smart_rollup::{
        host::Runtime,
        michelson::{
            ticket::FA2_1Ticket, MichelsonContract, MichelsonNat, MichelsonOption,
            MichelsonPair,
        },
        outbox::{OutboxMessageFull, OutboxMessageTransaction},
        storage::path::OwnedPath,
        types::{Contract, Entrypoint},
    };
    use tezos_smart_rollup_mock::MockHost;

    use crate::kv::{
        outbox::{OutboxMessage, PersistentOutboxQueue},
        Storage,
    };

    use super::Transaction;

    fn make_withdrawal(account: &PublicKeyHash) -> OutboxMessage {
        let creator =
            Contract::from_b58check("KT1NgXQ6Mwu3XKFDcKdYFS6dkkY3iNKdBKEc").unwrap();
        let parameters = MichelsonPair(
            MichelsonContract(Contract::try_from(account.to_base58()).unwrap()),
            FA2_1Ticket::new(
                creator.clone(),
                MichelsonPair(MichelsonNat::from(0), MichelsonOption(None)),
                10,
            )
            .unwrap(),
        );
        let outbox_tx = OutboxMessageTransaction {
            parameters,
            destination: creator,
            entrypoint: Entrypoint::try_from("burn".to_string()).unwrap(),
        };
        OutboxMessage::Withdrawal(vec![outbox_tx].into())
    }

    #[test]
    fn test_nested_transactions() {
        let hrt = &mut MockHost::default();
        let tx = &mut Transaction::default();

        #[derive(Clone, Serialize, Deserialize, Debug, Default, Encode, Decode)]
        struct Account {
            amount: u64,
        }

        impl Account {
            fn path(name: &str) -> OwnedPath {
                OwnedPath::try_from(format!("/jstz_account/{}", name)).unwrap()
            }

            fn get<'a>(
                hrt: &impl Runtime,
                tx: &'a mut Transaction,
                path: &OwnedPath,
            ) -> &'a mut Self {
                tx.entry(hrt, path.clone()).unwrap().or_insert_default()
            }

            fn get_from_storage(hrt: &impl Runtime, path: &OwnedPath) -> Self {
                Storage::get::<Account>(hrt, path)
                    .unwrap()
                    .unwrap_or_default()
            }
        }

        // Start transaction (tx0)
        tx.begin();

        let account1 = &Account::path("tz1notanaddress1");
        let account2 = &Account::path("tz1notanaddress2");

        assert_eq!(0, Account::get(hrt, tx, account1).amount);
        assert_eq!(0, Account::get(hrt, tx, account2).amount);

        // Start transaction (tx1)
        tx.begin();

        Account::get(hrt, tx, account2).amount += 25;

        assert_eq!(0, Account::get(hrt, tx, account1).amount);
        assert_eq!(25, Account::get(hrt, tx, account2).amount);

        // Start transaction (tx2)
        tx.begin();

        Account::get(hrt, tx, account1).amount += 57;

        assert_eq!(57, Account::get(hrt, tx, account1).amount);
        assert_eq!(25, Account::get(hrt, tx, account2).amount);

        // Commit transaction (tx2)
        tx.commit(hrt).unwrap();

        // In transaction (tx1)

        Account::get(hrt, tx, account1).amount += 57;

        assert_eq!(2 * 57, Account::get(hrt, tx, account1).amount);
        assert_eq!(25, Account::get(hrt, tx, account2).amount);

        // Commit transaction (tx1)
        tx.commit(hrt).unwrap();

        // In transaction (tx0)

        assert_eq!(2 * 57, Account::get(hrt, tx, account1).amount);

        Account::get(hrt, tx, account1).amount += 57;

        assert_eq!(3 * 57, Account::get(hrt, tx, account1).amount);

        tx.commit(hrt).unwrap();

        // Check storage

        assert_eq!(3 * 57, Account::get_from_storage(hrt, account1).amount);
        assert_eq!(25, Account::get_from_storage(hrt, account2).amount);
    }

    #[test]
    fn push_outbox_message_succeeds_until_outbox_queue_is_full() {
        let mut host = MockHost::default();
        let mut tx = Transaction {
            persistent_outbox: PersistentOutboxQueue::try_new(&mut host, 120).unwrap(),
            ..Transaction::default()
        };

        for i in 0..120 {
            if i % 10 == 0 {
                tx.begin();
            }
            let acc = PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            let message = make_withdrawal(&acc);
            tx.queue_outbox_message(&mut host, message).unwrap();
        }

        assert_eq!(120, tx.snapshot_outbox_len);

        // Adding an additional message to a full outbox queue without
        // flushing should fail
        let error = tx
            .queue_outbox_message(
                &mut host,
                make_withdrawal(
                    &PublicKeyHash::digest("failure account".to_string().as_bytes())
                        .unwrap(),
                ),
            )
            .expect_err("Outbox should be full");

        assert!(matches!(
            error,
            crate::error::Error::OutboxError {
                source: crate::kv::outbox::OutboxError::OutboxQueueFull
            }
        ));
    }

    #[test]
    fn non_final_commit_appends_outbox_messages_to_previous_snapshot() {
        let mut host = MockHost::default();
        let mut tx = Transaction {
            persistent_outbox: PersistentOutboxQueue::try_new(&mut host, 120).unwrap(),
            ..Transaction::default()
        };

        for i in 0..120 {
            if i % 60 == 0 {
                tx.begin();
            }
            let acc = PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            let message = make_withdrawal(&acc);
            tx.queue_outbox_message(&mut host, message).unwrap();
        }

        tx.commit(&mut host).unwrap();

        assert_eq!(120, tx.snapshot_outbox_len);

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(0, outbox.len());
    }

    #[test]
    #[ignore]
    fn final_commit_resets_snapshot_queue_len() {
        let mut host = MockHost::default();
        let mut tx = Transaction {
            persistent_outbox: PersistentOutboxQueue::try_new(&mut host, 120).unwrap(),
            ..Transaction::default()
        };

        for i in 0..120 {
            if i % 60 == 0 {
                tx.begin();
            }
            let acc = PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            let message = make_withdrawal(&acc);
            tx.queue_outbox_message(&mut host, message).unwrap();
        }

        tx.commit(&mut host).unwrap();
        tx.commit(&mut host).unwrap();
        assert_eq!(0, tx.snapshot_outbox_len);

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(100, outbox.len());
    }

    #[test]
    fn final_commit_flush_outbox_messages_in_enqueue_order() {
        let mut host = MockHost::default();
        let mut tx = Transaction {
            persistent_outbox: PersistentOutboxQueue::try_new(&mut host, 120).unwrap(),
            ..Transaction::default()
        };

        // Enqueue 120 messages, 60 per snapshot
        for i in 0..120 {
            if i % 60 == 0 {
                tx.begin();
            }

            let acc = PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap();
            let message = make_withdrawal(&acc);
            tx.queue_outbox_message(&mut host, message).unwrap();
        }

        // Commit both snapshots
        tx.commit(&mut host).unwrap();
        tx.commit(&mut host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        // Maximum number of outbox messages per level is 100.
        // The remaining 20 messages are left in the persistent queue.
        assert_eq!(100, outbox.len());
        assert_eq!(20, tx.persistent_outbox.len(&mut host).unwrap());

        for (i, outbox_message) in outbox.iter().enumerate() {
            let (_, message) =
                OutboxMessageFull::<OutboxMessage>::nom_read(outbox_message.as_slice())
                    .unwrap();

            assert_eq!(
                message,
                make_withdrawal(
                    &PublicKeyHash::digest(format!("account{}", i).as_bytes()).unwrap()
                )
                .into()
            );
        }

        tx.begin();
        tx.commit(&mut host).unwrap();

        let level = host.run_level(|_| {});
        let outbox = host.outbox_at(level);

        assert_eq!(20, outbox.len());
        assert_eq!(0, tx.persistent_outbox.len(&mut host).unwrap());

        for (i, outbox_message) in outbox.iter().enumerate().take(20) {
            let (_, message) =
                OutboxMessageFull::<OutboxMessage>::nom_read(outbox_message.as_slice())
                    .unwrap();

            assert_eq!(
                message,
                make_withdrawal(
                    &PublicKeyHash::digest(format!("account{}", 100 + i).as_bytes())
                        .unwrap()
                )
                .into()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use bincode::{Decode, Encode};
    use serde::{Deserialize, Serialize};
    use tezos_smart_rollup_mock::MockHost;

    use super::*;
    #[derive(Debug, Default, Clone, Serialize, Deserialize, Encode, Decode)]
    struct TestValue(i32);

    #[test]
    fn test_entry_or_insert_default() {
        let hrt = &MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();

        // Create a test path
        let try_from = OwnedPath::try_from("/test".to_string());
        let path = try_from.unwrap();

        // Test or_insert_default on vacant entry
        let entry = tx.entry::<TestValue>(hrt, path.clone()).unwrap();
        let value = entry.or_insert_default();
        assert_eq!(value.0, 0);

        // Test or_insert_default on occupied entry
        let entry = tx.entry::<TestValue>(hrt, path.clone()).unwrap();
        entry.or_insert_default().0 = 42;
        // Get entry again, should return existing value
        let entry = tx.entry::<TestValue>(hrt, path).unwrap();
        let value = entry.or_insert_default();
        assert_eq!(value.0, 42);
    }

    #[test]
    fn test_entry_or_insert_with() {
        let hrt = &MockHost::default();
        let mut tx = Transaction::default();
        tx.begin();

        let path = OwnedPath::try_from("/test".to_string()).unwrap();

        // Test or_insert_with on vacant entry
        let entry = tx.entry::<TestValue>(hrt, path.clone()).unwrap();
        let value = entry.or_insert_with(|| TestValue(100));
        assert_eq!(value.0, 100); // Custom value should be used

        // Test or_insert_with on occupied entry
        let entry = tx.entry::<TestValue>(hrt, path.clone()).unwrap();
        let value = entry.or_insert_with(|| TestValue(200));
        assert_eq!(value.0, 100); // Should keep existing value, not call closure

        // Test closure is not called for occupied entry
        let mut called = false;
        let entry = tx.entry::<TestValue>(hrt, path).unwrap();
        let _value = entry.or_insert_with(|| {
            called = true;
            TestValue(300)
        });
        assert!(!called, "Closure should not be called for occupied entry");
    }
}
