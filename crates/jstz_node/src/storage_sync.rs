use std::{path::PathBuf, time::Duration};

use crate::sequencer::{self, db::Db};
use anyhow::Result;
use futures_util::StreamExt;
use jstz_core::kv::storage_update::{BatchStorageUpdate, StorageUpdate};
use jstz_utils::{
    event_stream::EventStream,
    retry::{exponential_backoff, retry_async},
};
use log::error;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use tokio::task::AbortHandle;

pub struct StorageSync {
    abort_handle: AbortHandle,
}

impl StorageSync {
    #[allow(dead_code)]
    pub async fn spawn(db: Db, log_path: PathBuf) -> Result<Self> {
        let mut stream = EventStream::<BatchStorageUpdate>::from_file(log_path).await?;
        let abort_handle = {
            let task = tokio::task::spawn(async move {
                while let Some(mb_updates) = stream.next().await {
                    let db = db.clone();
                    match mb_updates {
                        Ok(updates) => {
                            if let Err(e) = apply_batch_with_retry(db, updates).await {
                                error!(
                                    "storage_sync: failed to apply storage updates, aborting task. error: {e}"
                                );
                                // NOTE: In the future, we may want to support a "degraded mode" here.
                                // See https://linear.app/tezos/issue/JSTZ-888/sequencer-reliability for discussion.
                                // If the DB fails after retries, the node could enter degraded mode, reject API requests,
                                // and buffer updates to a WAL for later replay when the DB is restored.
                                break;
                            }
                        }
                        Err(e) => {
                            error!("storage_sync: failed to read storage updates stream. error: {e}")
                        }
                    }
                }
            });
            task.abort_handle()
        };

        Ok(Self { abort_handle })
    }
}

/// Applies a batch of storage updates with exponential backoff.
/// The retry is limited to 6 attempts with a maximum delay of 5 seconds.
async fn apply_batch_with_retry(db: Db, updates: BatchStorageUpdate) -> Result<()> {
    retry_async(
        exponential_backoff(200, 5, Duration::from_secs(6)),
        || async {
            let conn = db.connection();
            match conn {
                Ok(conn) => apply_batch_tx(conn, updates.clone()).await,
                Err(e) => Err(e),
            }
        },
        |_| true,
    )
    .await
}

/// Executes a batch of storage updates in a single transaction.
async fn apply_batch_tx(
    mut conn: PooledConnection<SqliteConnectionManager>,
    updates: BatchStorageUpdate,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let tx = conn.transaction()?;
        for update in updates {
            let res = match update {
                StorageUpdate::Insert { ref key, ref value } => {
                    sequencer::db::exec_write(&tx, key, &hex::encode(value))
                }
                StorageUpdate::Remove { ref key } => {
                    sequencer::db::exec_delete(&tx, key).map(|_| ())
                }
            };
            if let Err(e) = res {
                error!("error writing storage update {:?} {e}", update);
                return Err(e);
            }
        }
        tx.commit()?;
        Ok(())
    })
    .await?
}

impl Drop for StorageSync {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temp_db;
    use anyhow::Result;
    use bincode::{Decode, Encode};
    use jstz_core::{event::Event, BinEncodable};
    use jstz_utils::test_util::append_async;
    use serde::{Deserialize, Serialize};
    use std::io::Write;
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tezos_smart_rollup::storage::path::OwnedPath;
    use tokio::{
        task::yield_now,
        time::{sleep, timeout},
    };

    #[derive(Debug, Serialize, Deserialize, PartialEq, Encode, Decode, Clone)]
    struct DummyValue(u32);

    fn mock_key() -> OwnedPath {
        OwnedPath::try_from("/foo".to_string()).unwrap()
    }

    fn mock_insert_event() -> BatchStorageUpdate {
        let mut event = BatchStorageUpdate::new(1);
        let key = mock_key();
        let val1 = DummyValue(42);
        let _ = event.push_insert(&key, &val1);
        event
    }

    fn mock_remove_event() -> BatchStorageUpdate {
        let mut event = BatchStorageUpdate::new(1);
        let key = mock_key();
        event.push_remove(&key);
        event
    }

    fn make_line<T: Event + Serialize>(event: &T) -> String {
        format!("[{}] {}", T::tag(), serde_json::to_string(event).unwrap())
    }

    #[tokio::test]
    async fn process_storage_sync() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let file_path = tmp.path().to_path_buf();
        let (db, _db_file) = temp_db().unwrap();

        let _storage_sync = StorageSync::spawn(db.clone(), file_path.clone()).await?;
        let line = make_line(&mock_insert_event());

        // `StorageUpdate::Insert` is picked up and reflected in the database
        let writer = tokio::spawn(append_async(file_path.clone(), line, 25));
        timeout(Duration::from_secs(1), async {
            loop {
                if let Some(value) = db.read_key(&mock_key().to_string()).unwrap() {
                    let decoded = hex::decode(value).unwrap();
                    let dummy_value: DummyValue =
                        BinEncodable::decode(decoded.as_slice()).expect("deserialize");
                    assert_eq!(dummy_value, DummyValue(42));
                    break;
                }
                yield_now().await;
            }
        })
        .await?;
        writer.await??;

        // `StorageUpdate::Remove` is picked up and reflected in the database
        let line = make_line(&mock_remove_event());
        let writer = tokio::spawn(append_async(file_path, line, 25));
        let res = timeout(Duration::from_secs(1), async {
            loop {
                if db
                    .key_exists(&mock_key().to_string())
                    .is_ok_and(|exists| !exists)
                {
                    break;
                }
                yield_now().await;
            }
        })
        .await;
        writer.await??;
        assert!(res.is_ok(), "The key wasn't removed");

        Ok(())
    }

    #[tokio::test]
    async fn ignores_noise_lines() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let file_path = tmp.path().to_path_buf();
        let (db, _db_file) = temp_db().unwrap();

        let _storage_sync = StorageSync::spawn(db.clone(), file_path.clone()).await?;

        // `StorageUpdate::Insert`` is picked up and reflected in the database
        let writer = tokio::spawn(append_async(
            file_path.clone(),
            "noise_line".to_string(),
            25,
        ));
        sleep(Duration::from_secs(1)).await;
        // nothing was inserted to the database
        let count = db.count_subkeys("").unwrap();
        writer.await??;
        assert!(count.is_none(), "Noise line should not affect the database");

        Ok(())
    }

    #[tokio::test]
    async fn ignores_preexisting_lines() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        let file_path = tmp.path().to_path_buf();
        let (db, _db_file) = temp_db().unwrap();
        writeln!(tmp.as_file_mut(), "{}", make_line(&mock_insert_event()))?;
        tmp.as_file_mut().sync_all()?;
        let _storage_sync = StorageSync::spawn(db.clone(), file_path.clone()).await?;
        sleep(Duration::from_secs(1)).await;
        // nothing was inserted to the database
        let count = db.count_subkeys("").unwrap();
        assert!(
            count.is_none(),
            "Preexisting lines should not affect the database"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_apply_batch_tx_insert_and_remove() -> Result<()> {
        let (db, _db_file) = temp_db().unwrap();
        let key1 = mock_key();
        let key2 = OwnedPath::try_from("/bar".to_string()).unwrap();
        let value1 = DummyValue(123);
        let value2 = DummyValue(456);
        let mut batch = BatchStorageUpdate::new(3);
        let _ = batch.push_insert(&key1, &value1);
        let _ = batch.push_insert(&key2, &value2);
        batch.push_remove(&key1);
        let conn = db.connection().unwrap();
        // Should succeed
        apply_batch_tx(conn, batch).await?;
        // After transaction, key1 should not exist, key2 should exist with correct value
        assert!(db.read_key(&key1.to_string())?.is_none());
        let value = db.read_key(&key2.to_string())?.expect("key2 should exist");
        let decoded: DummyValue =
            BinEncodable::decode(&hex::decode(value).unwrap()).unwrap();
        assert_eq!(decoded, value2);
        Ok(())
    }

    #[tokio::test]
    async fn test_apply_batch_tx_atomicity() -> Result<()> {
        let (db, _db_file) = temp_db().unwrap();
        let key1 = mock_key();
        let value1 = DummyValue(123);
        let mut batch = BatchStorageUpdate::new(2);
        let _ = batch.push_insert(&key1, &value1);
        // Intentionally create an invalid update
        let invalid_key =
            unsafe { OwnedPath::from_bytes_unchecked(vec![0; 1_000_000_000]) };
        let value2 = DummyValue(456);
        let _ = batch.push_insert(&invalid_key, &value2);
        let conn = db.connection().unwrap();
        let result = apply_batch_tx(conn, batch).await;
        assert!(
            result.is_err(),
            "Transaction with invalid update should fail"
        );
        // Ensure nothing was stored
        assert!(
            !db.key_exists(&key1.to_string()).unwrap(),
            "No data should be stored if transaction fails"
        );
        Ok(())
    }
}
