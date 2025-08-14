//! # KV Storage Update Event
//!
//! This module defines storage update events for the key-value store.
//! These events are leaked to the kernel debug log, allowing other
//! components (e.g., the sequencer) to observe and react to storage changes.

use crate::event::Event;
use crate::kv::Value;
use crate::{error::Result, event::EventPublish};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};
use tezos_smart_rollup::host::Runtime;
use tezos_smart_rollup_host::path::Path;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
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

/// Storage update event.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
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

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Publishes the event if it is not empty.
    /// Returns `Ok(())` if the event is empty or was published successfully.
    pub fn publish_event<R>(self, rt: &R) -> crate::event::Result<()>
    where
        R: Runtime,
    {
        if self.is_empty() {
            return Ok(());
        }
        <Self as EventPublish>::publish_event(self, rt)
    }
}

impl IntoIterator for BatchStorageUpdate {
    type Item = StorageUpdate;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::{Decode, Encode};
    use serde::{Deserialize, Serialize};
    use tezos_smart_rollup_host::path::OwnedPath;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Encode, Decode, Clone)]
    struct DummyValue(u32);

    #[test]
    fn test_batch_storage_update_insert_and_remove() {
        let mut batch = BatchStorageUpdate::new(2);
        let key1 = OwnedPath::try_from("/key1".to_string()).unwrap();
        let val1 = DummyValue(42);
        batch.push_insert(&key1, &val1).unwrap();
        batch.push_remove(&key1);

        assert_eq!(batch.0.len(), 2);
        match &batch.0[0] {
            StorageUpdate::Insert { key, .. } => assert_eq!(key, "/key1"),
            _ => panic!("Expected Insert variant"),
        }
        match &batch.0[1] {
            StorageUpdate::Remove { key } => assert_eq!(key, "/key1"),
            _ => panic!("Expected Remove variant"),
        }
    }

    #[test]
    fn test_batch_storage_update_is_empty() {
        let mut batch = BatchStorageUpdate::new(1);
        assert!(batch.is_empty());
        let key = OwnedPath::try_from("/key".to_string()).unwrap();
        batch.push_remove(&key);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_publish_event() {
        use crate::event::test::Sink;
        use tezos_smart_rollup_mock::MockHost;

        let mut sink = Sink(Vec::new());
        let mut host = MockHost::default();
        host.set_debug_handler(unsafe {
            std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                &mut sink.0,
            )
        });

        // Non-empty batch should be published
        let key = OwnedPath::try_from("/key".to_string()).unwrap();
        let val = DummyValue(123);
        let mut batch = BatchStorageUpdate::new(1);
        batch.push_insert(&key, &val).unwrap();
        batch.publish_event(&host).unwrap();
        let lines = sink.lines();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("[BATCH_STORAGE_UPDATE]")),
            "Event not published"
        );

        // Empty batch should not be published
        let empty_batch = BatchStorageUpdate::new(0);
        let prev_len: usize = sink.lines().len();
        empty_batch.publish_event(&host).unwrap();
        let after_len = sink.lines().len();
        assert_eq!(
            prev_len, after_len,
            "No event should be published for empty batch"
        );
    }
}
