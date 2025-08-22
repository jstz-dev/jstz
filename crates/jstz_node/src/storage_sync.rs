use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use std::{path::PathBuf, time::Duration};

use crate::sequencer::{self, db::Db};
use anyhow::{Context as _, Result};
use futures_util::StreamExt;
use jstz_core::kv::storage_update::{BatchStorageUpdate, StorageUpdate};
use jstz_utils::event_stream::EventStream;
use jstz_utils::retry::{exponential_backoff, retry_async};
use log::{error, warn};
use tokio::sync::oneshot::{self, Receiver, Sender};

/// A handle to a long-running background worker that consumes an event stream and
/// applies storage updates to the database.
///
/// When this handle is dropped, it sends a shutdown signal to the worker and waits for the thread to terminate.  
/// Awaiting the handle completes when the worker exits. If the worker exits unexpectedly (e.g., due to a DB error or stream failure),
/// it returns an error.
/// Note: Dropping this handle does **not** initiate a shutdown; it only observes and waits for the worker's termination.
///
pub struct StorageSync {
    inner: Option<thread::JoinHandle<()>>,
    kill_tx: Option<Sender<()>>,
    death_rx: Receiver<Result<()>>,
}

impl Future for StorageSync {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = Pin::new(&mut self.get_mut().death_rx);
        this.poll(cx).map(|r| match r {
            Ok(res) => res,
            Err(e) => Err(e.into()),
        })
    }
}
/// Spawns a new storage sync worker.
/// The thread will read the event stream file and apply the storage updates to the database.
///
/// # Arguments
///
/// * `db` - The database to use.
/// * `log_path` - The path to the event stream file.
pub fn spawn(
    db: Db,
    log_path: PathBuf,
    #[cfg(test)] on_kill: impl FnOnce() + Send + 'static,
) -> Result<StorageSync> {
    let (kill_tx, mut kill_rx) = oneshot::channel();
    let (death_tx, death_rx) = oneshot::channel();
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .context("failed to build tokio runtime")?;

    let handle = thread::spawn(move || {
        let res = tokio_rt.block_on(async move {
                let mut stream =
                    match EventStream::<BatchStorageUpdate>::from_file(log_path).await {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to open event stream: {e}");
                            return Err(e);
                        }
                    };
                    loop {
                        tokio::select! {
                            _ = &mut kill_rx => {
                                #[cfg(test)]
                                on_kill();
                                return Ok(());
                            }
                            next_item = stream.next() => {
                                match next_item {
                                    Some(Ok(updates)) => {
                                        if let Err(e) = apply_batch_tx_with_retry(&db, updates).await {
                                            error!("db error, aborting: {e}");
                                            return Err(e);
                                        }
                                    }
                                    Some(Err(e)) => {
                                        error!("event stream error, aborting: {e}");
                                        return Err(e);
                                    }
                                    None => {
                                        warn!("stream ended, aborting");
                                        return Err(anyhow::anyhow!("stream ended"));
                                    }
                                }
                            }
                        }
                    }
            });
        let _ = death_tx.send(res);
    });
    Ok(StorageSync {
        kill_tx: Some(kill_tx),
        death_rx,
        inner: Some(handle),
    })
}

impl Drop for StorageSync {
    fn drop(&mut self) {
        if let Some(kill_sig) = self.kill_tx.take() {
            let _ = kill_sig.send(());
        }
        if let Some(handle) = self.inner.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
impl StorageSync {
    pub fn kill(&mut self) {
        if let Some(kill_sig) = self.kill_tx.take() {
            let _ = kill_sig.send(());
        }
    }
}

/// Applies a batch of storage updates with exponential backoff.
/// The retry is limited to 6 attempts with a maximum delay of 8 seconds.
async fn apply_batch_tx_with_retry(db: &Db, updates: BatchStorageUpdate) -> Result<()> {
    retry_async(
        exponential_backoff(50, 6, Duration::from_secs(8)),
        || async { apply_batch_tx(db, updates.clone()) },
        |_| true,
    )
    .await
}

/// Executes a batch of storage updates in a single transaction.
/// This function is blocking but it's called from a separate thread so it's ok.
fn apply_batch_tx(db: &Db, updates: BatchStorageUpdate) -> Result<()> {
    let mut conn = db.connection()?;
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

        let _storage_sync = spawn(db.clone(), file_path.clone(), || {}).unwrap();
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
        let _storage_sync = spawn(db.clone(), file_path.clone(), || {}).unwrap();
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
        let _storage_sync = spawn(db.clone(), file_path.clone(), || {}).unwrap();
        sleep(Duration::from_secs(1)).await;
        // nothing was inserted to the database
        let count = db.count_subkeys("").unwrap();
        assert!(
            count.is_none(),
            "Preexisting lines should not affect the database"
        );
        Ok(())
    }

    #[test]
    fn test_apply_batch_tx_insert_and_remove() -> Result<()> {
        let (db, _db_file) = temp_db().unwrap();
        let key1 = mock_key();
        let key2 = OwnedPath::try_from("/bar".to_string()).unwrap();
        let value1 = DummyValue(123);
        let value2 = DummyValue(456);
        let mut batch = BatchStorageUpdate::new(3);
        let _ = batch.push_insert(&key1, &value1);
        let _ = batch.push_insert(&key2, &value2);
        batch.push_remove(&key1);
        // Should succeed
        apply_batch_tx(&db, batch)?;
        // After transaction, key1 should not exist, key2 should exist with correct value
        assert!(db.read_key(&key1.to_string())?.is_none());
        let value = db.read_key(&key2.to_string())?.expect("key2 should exist");
        let decoded: DummyValue =
            BinEncodable::decode(&hex::decode(value).unwrap()).unwrap();
        assert_eq!(decoded, value2);
        Ok(())
    }

    #[test]
    fn test_apply_batch_tx_atomicity() -> Result<()> {
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
        let result = apply_batch_tx(&db, batch);
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

    #[test]
    fn storage_sync_drop() {
        use std::sync::{Arc, Mutex};
        let v = Arc::new(Mutex::new(0));
        let cp = v.clone();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let file_path = tmp.path().to_path_buf();
        let (db, _db_file) = crate::temp_db().unwrap();
        let storage_sync = spawn(db, file_path, move || {
            *cp.lock().unwrap() += 1;
        })
        .unwrap();
        // Drop the storage_sync to trigger shutdown
        drop(storage_sync);
        assert_eq!(*v.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn await_resolves_when_worker_exits() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let file_path = tmp.path().to_path_buf();
        let (db, _db_file) = temp_db().unwrap();

        // Does not resolve while the worker is running
        let storage_sync = spawn(db.clone(), file_path.clone(), || {}).unwrap();
        let res = tokio::time::timeout(Duration::from_secs(1), storage_sync).await;
        assert!(res.is_err(), "Worker unexpectedly exited");

        // The worker should exit and .await should resolve
        let mut storage_sync = spawn(db.clone(), file_path, || {}).unwrap();
        storage_sync.kill();
        let res = tokio::time::timeout(Duration::from_secs(1), storage_sync).await;
        assert!(res.is_ok(), "Worker did not exit");

        // The worker should exit and propagate the error
        let fake_path = PathBuf::from("/fake/path");
        let storage_sync = spawn(db, fake_path, || {}).unwrap();
        let res = tokio::time::timeout(Duration::from_secs(1), storage_sync)
            .await
            .unwrap();
        assert!(res.is_err_and(|e| e.to_string().contains("No such file")));

        Ok(())
    }
}
