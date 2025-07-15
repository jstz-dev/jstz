#![allow(dead_code)]
use futures::{stream, Stream, StreamExt};
use jstz_proto::BlockLevel;
use log::error;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Storage error: {0}")]
    StorageError(#[from] anyhow::Error),
    #[error("Stream error: {0}")]
    StreamError(String),
}

#[async_trait::async_trait]
pub trait BlockProgressStore: Clone {
    async fn last_level(&self) -> Result<Option<BlockLevel>>;
    async fn set_last_level(&self, level: BlockLevel) -> Result<()>;
}

const STORE_KEY: &str = "/block_progress";

/// Persists the last processed block level processed for inbox messages.
#[derive(Clone)]
pub struct FileBlockProgressStore {
    file: Arc<File>,
}

impl FileBlockProgressStore {
    pub fn new(file: File) -> Self {
        Self {
            file: Arc::new(file),
        }
    }
}

#[async_trait::async_trait]
impl BlockProgressStore for FileBlockProgressStore {
    async fn last_level(&self) -> Result<Option<BlockLevel>> {
        let file = Arc::as_ref(&self.file);
        let mut file = file;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| Error::StorageError(e.into()))?;
        let mut buf = [0u8; 8];
        match file.read_exact(&mut buf) {
            Ok(_) => {
                let level = u64::from_le_bytes(buf);
                Ok(Some(level))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(Error::StorageError(e.into())),
        }
    }

    async fn set_last_level(&self, level: BlockLevel) -> Result<()> {
        let file = Arc::as_ref(&self.file);
        let mut file = file;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| Error::StorageError(e.into()))?;
        file.set_len(0).map_err(|e| Error::StorageError(e.into()))?;
        let buf = (level).to_le_bytes();
        file.write_all(&buf)
            .map_err(|e| Error::StorageError(e.into()))?;
        file.flush().map_err(|e| Error::StorageError(e.into()))?;
        Ok(())
    }
}

/// A block level that, once processed, should be committed.
pub(crate) struct BlockLevelToCommit<S>
where
    S: BlockProgressStore,
{
    level: BlockLevel,
    store: S,
}

impl<S> BlockLevelToCommit<S>
where
    S: BlockProgressStore,
{
    pub fn new(store: S, level: BlockLevel) -> Self {
        Self { level, store }
    }

    // Commit once the level is processed.
    pub async fn commit(self) -> Result<()> {
        self.store.set_last_level(self.level).await
    }

    pub fn level(&self) -> BlockLevel {
        self.level
    }
}

/// A stream that yields block levels (e.g., 123, 124, 125...) from an L1 chain,
/// If it misses some blocks (e.g., because sequencer restarted or the network failed), it will catch up by yielding all missing levels in order.
/// It uses a BlockProgressStore to track the last successfully processed block level.
/// When resumed (e.g., after a restart or network failure), the stream starts from last_level + 1, yielding all missing block levels in order.
/// In case of network failure, the stream will retry to fetch the block level from the live stream in an exponential backoff manner.
///
/// **Note:** After processing a level, the client must commit the level to the store to mark it as handled.
struct StreamState<S, F, L>
where
    S: BlockProgressStore,
    F: Fn() -> anyhow::Result<L> + Clone + Send + 'static,
    L: Stream<Item = anyhow::Result<BlockLevel>> + Unpin + Send + 'static,
{
    catch_up: VecDeque<BlockLevel>,
    live_stream: Option<Pin<Box<L>>>,
    store: S,
    mk_live_stream: F,
}

pub fn create_sequential_block_stream<S, F, L>(
    store: S,
    mk_live_stream: F,
) -> impl Stream<Item = Result<BlockLevelToCommit<S>>>
where
    S: BlockProgressStore + 'static,
    F: Fn() -> anyhow::Result<L> + Clone + Send + 'static,
    L: Stream<Item = anyhow::Result<BlockLevel>> + Unpin + Send + 'static,
{
    let state = StreamState {
        catch_up: VecDeque::new(),
        live_stream: None,
        store,
        mk_live_stream,
    };

    stream::unfold(state, |mut state| async move {
        // If we have catch-up levels queued, yield them first
        if let Some(level) = state.catch_up.pop_front() {
            let item = Ok(BlockLevelToCommit::new(state.store.clone(), level));
            return Some((item, state));
        }

        // Ensure live stream is connected (retry if needed)
        if state.live_stream.is_none() {
            let retry_strategy =
                ExponentialBackoff::from_millis(200).max_delay(Duration::from_secs(5));
            let stream =
                Retry::spawn(retry_strategy, || async { (state.mk_live_stream)() })
                    .await
                    .unwrap();
            state.live_stream = Some(Box::pin(stream))
        }

        // Try to pull the next live block
        match state.live_stream.as_mut().unwrap().next().await {
            Some(Ok(level)) => {
                let last_level = match state.store.last_level().await {
                    Ok(Some(last_level)) => last_level,
                    Ok(None) => level,
                    Err(e) => {
                        return Some((Err(Error::StorageError(e.into())), state));
                    }
                };

                if last_level + 1 < level {
                    state.catch_up = (last_level + 2..=level).collect();
                    return Some((
                        Ok(BlockLevelToCommit::new(state.store.clone(), last_level + 1)),
                        state,
                    ));
                }
                let item = Ok(BlockLevelToCommit::new(state.store.clone(), level));
                Some((item, state))
            }
            Some(Err(e)) => {
                // Drop the stream and let the retry handle it next time
                state.live_stream = None;
                Some((Err(Error::StreamError(e.to_string())), state))
            }
            None => {
                state.live_stream = None;
                Some((
                    Err(Error::StreamError("Stream ended unexpectedly".to_string())),
                    state,
                ))
            }
        }
    })
}

// #[cfg(test)]
// mod tests {
//     use crate::sequencer::{
//         db::Db,
//         inbox::stream::{BlockProgressStore, DbBlockProgressStore},
//     };

//     #[test]
//     fn block_progress_store() {
//         let db = Db::init(Some("")).unwrap();
//         let store = DbBlockProgressStore::new(db);
//         assert_eq!(store.last_level().unwrap(), None);
//         store.set_last_level(1).unwrap();
//         assert_eq!(store.last_level().unwrap(), Some(1));
//     }
// }
