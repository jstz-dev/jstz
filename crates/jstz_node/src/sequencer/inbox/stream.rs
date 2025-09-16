#![allow(dead_code)]
use crate::sequencer::inbox::store::{CheckpointLoadFuture, CheckpointStore};
use futures_util::{stream::BoxStream, FutureExt, Stream, StreamExt};
use jstz_proto::BlockLevel;
use log::error;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, time::Duration};
use tokio::time::{sleep, Sleep};

/// A stream of live block levels.
type SourceStream = BoxStream<'static, anyhow::Result<BlockLevel>>;

/// Trait that produces a new source stream when called.
/// Used to (re)connect to the live block stream as needed.
pub trait StreamFactory: Fn() -> SourceStream + Send + 'static {}
impl<T: Fn() -> SourceStream + Send + 'static> StreamFactory for T {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error occurred during checkpoint I/O.
    #[error("Checkpoint I/O error: {0}")]
    CheckpointIo(#[from] io::Error),
}

/// Represents a block level that must be committed once processed.
pub struct PendingBlock<S> {
    level: BlockLevel,
    store: S,
}

impl<S: CheckpointStore> PendingBlock<S> {
    pub fn new(store: S, level: BlockLevel) -> Self {
        Self { level, store }
    }

    /// Mark this block as processed by saving its checkpoint.
    pub async fn commit(&mut self) -> Result<()> {
        self.store.save(self.level).await?;
        Ok(())
    }

    /// Get the block level.
    pub fn level(&self) -> BlockLevel {
        self.level
    }
}

#[cfg(test)]
const RETRY_MS: u64 = 150;

#[cfg(not(test))]
const RETRY_MS: u64 = 2000;

#[derive(Clone, Copy)]
enum State {
    /// Waiting a fixed delay due to io error from the source stream.
    Backoff,
    /// Actively polling the live source stream.
    Streaming,
    /// Emitting a backlog of block levels in the range [left, right).
    Backlog((BlockLevel, BlockLevel)),
    /// Loading the checkpoint from the store to compare with the live block to determine the next block to yield.
    LoadingCheckpoint { live: BlockLevel },
}

/// # SequentialBlockStream
///
/// This stream provides gap-filling, resumable block level streaming for the inbox monitor,
/// yielding block levels in order while handling gaps, source stream reconnection, and block progress persistence.
///
/// ## Example
/// ```rust
/// let stream = SequentialBlockStream::new(store, factory);
/// pin_mut!(stream);
/// while let Some(pending_block) = stream.next().await {
///     if let Ok(pending_block) = pending_block {
///         // Process the block
///         pending_block.commit().await?; // Persist progress
///     }
/// }
/// ```
///
/// ## State transitions
///
///     Streaming (follow live source)
///         |
///         | source error / end
///         |--> Backoff
///         |
///         | new block
///         v
///     LoadingCheckpoint { live }
///         |
///         | Err(e) from store
///         |--> return error, stay in LoadingCheckpoint
///         |
///         | Ok(None) or Ok(chk+1 == live)
///         |--> yield(live), go to Streaming
///         |
///         | Ok(chk+1 < live)
///         |--> Backlog(chk+1 .. live+1)
///         |
///         | Ok(chk >= live)
///         |--> Backoff
///         |
///         |
///         v
///         Ok(chk + 1 = live) / None: yield and go back to Streaming
///
///     Backlog(L..R)
///         | if L < R: yield(L), L++, stay in Backlog
///         | if L == R: go to Streaming
///
///     Backoff
///         | wait(delay) -> Streaming
///
/// ## Error Handling
/// - If the checkpoint store returns an error, the stream will retry indefinitely in LoadingCheckpoint.
/// - The client is responsible for handling checkpoint store errors.
/// - If the source stream ends or errors, the stream will automatically reconnect with backoff.
#[pin_project(project = SequentialBlockStreamProj)]
pub struct SequentialBlockStream<S, F> {
    /// Store for block checkpoints.
    store: S,
    /// Current state of the stream state machine.
    state: State,
    /// The current live source stream, or None if dropped for reconnect/backoff.
    source_stream: Option<SourceStream>,
    /// Factory to create a new source stream on demand.
    stream_factory: F,
    /// Future for loading the checkpoint.
    fut: Option<CheckpointLoadFuture>,
    /// The current delay for backoff, or None if dropped for reconnect/backoff.
    #[pin]
    delay: Option<Sleep>,
}

impl<S, F> SequentialBlockStream<S, F>
where
    F: StreamFactory,
{
    /// Create a new sequential block stream with the given store and stream factory.
    pub fn new(store: S, stream_factory: F) -> Self {
        Self {
            state: State::Streaming,
            store,
            source_stream: None,
            stream_factory,
            delay: None,
            fut: None,
        }
    }
}

impl<S, F> Stream for SequentialBlockStream<S, F>
where
    S: CheckpointStore,
    F: StreamFactory,
{
    type Item = Result<PendingBlock<S>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            println!("[poll_next] starting loop");
            let store = this.store.clone();
            let (next_state, ret) = match *this.state {
                // Waiting for a delay to pass before trying to reconnect to the source stream
                State::Backoff => this.handle_backoff(cx),
                // We know about a gap [left, right) and emit it sequentially.
                State::Backlog((left, right)) => this.handle_backlog(cx, left, right),
                // Poll the current block from the source stream and transition to checkpoint loading
                State::Streaming => this.handle_streaming(cx),
                // Awaiting a checkpoint store read to compare with `live` and determine the next block to yield.
                State::LoadingCheckpoint { live } => {
                    this.handle_loading_checkpoint(cx, live)
                }
            };
            *this.state = next_state;
            if let Some(ret) = ret {
                return ret;
            }
        }
    }
}

type PollResult<S, F> = Poll<Option<<SequentialBlockStream<S, F> as Stream>::Item>>;

impl<'a, S, F> SequentialBlockStreamProj<'a, S, F>
where
    S: CheckpointStore,
    F: StreamFactory,
{
    fn handle_backoff(
        &mut self,
        cx: &mut Context<'_>,
    ) -> (State, Option<PollResult<S, F>>) {
        println!("[handle_backoff] Entered Backoff");
        if self.delay.is_none() {
            self.delay.set(Some(sleep(Duration::from_millis(RETRY_MS))));
        }
        match self.delay.as_mut().as_pin_mut().unwrap().poll_unpin(cx) {
            Poll::Pending => (State::Backoff, Some(Poll::Pending)),
            Poll::Ready(()) => {
                self.delay.set(None);
                (State::Streaming, None)
            }
        }
    }

    fn handle_backlog(
        &mut self,
        cx: &mut Context<'_>,
        left: BlockLevel,
        right: BlockLevel,
    ) -> (State, Option<PollResult<S, F>>) {
        println!("[poll_next] Entered Backlog: left={left}, right={right}");
        // If there are more blocks in the backlog, yield the next one
        if left < right {
            let next = left;
            println!("[poll_next] Returning PendingBlock for backlog: {next}");
            (
                State::Backlog((next + 1, right)),
                Some(Poll::Ready(Some(Ok(PendingBlock::new(
                    self.store.clone(),
                    next,
                ))))),
            )
        } else {
            // Backlog exhausted, switch to the connected state to stream live blocks
            println!("[poll_next] Backlog exhausted, switching to Connected");
            (State::Streaming, None)
        }
    }

    fn handle_streaming(
        &mut self,
        cx: &mut Context<'_>,
    ) -> (State, Option<PollResult<S, F>>) {
        // Ensure we have a live stream, reconnect if needed
        let source_stream = self.source_stream.get_or_insert((self.stream_factory)());
        match source_stream.poll_next_unpin(cx) {
            // We have a new live block, compare with the local checkpoint
            Poll::Ready(Some(Ok(live))) => (State::LoadingCheckpoint { live }, None),
            // An error occurred, reconnect to the source stream
            Poll::Ready(err) => {
                let msg = match err {
                    Some(Err(e)) => format!("Block stream error: {e:?}"),
                    None => "Block stream ended unexpectedly".to_string(),
                    _ => unreachable!(),
                };
                error!("[handle_streaming] {msg}");
                // Drop the stream so we reconnect next time
                self.source_stream.take();
                (State::Backoff, None)
            }
            Poll::Pending => (State::Streaming, Some(Poll::Pending)),
        }
    }

    fn handle_loading_checkpoint(
        &mut self,
        cx: &mut Context<'_>,
        live: BlockLevel,
    ) -> (State, Option<PollResult<S, F>>) {
        println!("[handle_loading_checkpoint] Entered LoadCheckpoint for block {live}");
        let fut = self.fut.get_or_insert(self.store.load_fut());
        match fut.poll_unpin(cx) {
            Poll::Pending => {
                println!("[handle_loading_checkpoint] Checkpoint future pending");
                (State::LoadingCheckpoint { live }, Some(Poll::Pending))
            }
            Poll::Ready(Err(e)) => {
                println!("[handle_loading_checkpoint] Checkpoint future error: {e:?}");
                // Error loading checkpoint, return error
                // NOTE: unlikely to happen as checkpoint is saved in the file
                self.fut.take();
                (
                    State::LoadingCheckpoint { live },
                    Some(Poll::Ready(Some(Err(e.into())))),
                )
            }
            Poll::Ready(Ok(checkpoint)) => {
                println!("[handle_loading_checkpoint] Checkpoint ready: {checkpoint:?}");
                let (next_state, next_level) = match checkpoint {
                    None => (State::Streaming, Some(live)), // Cold start: follow live
                    Some(chk) if chk + 1 == live => (State::Streaming, Some(live)), // In sync
                    Some(chk) if chk + 1 < live => {
                        // Behind: queue the remaining gap into the backlog
                        (State::Backlog((chk + 1, live + 1)), None)
                    }
                    _ /* checkpoint >= live */ => {
                        // Duplicate/rewind: simplest policy—wait and reconnect.
                        // TODO: handle block reorg
                        // lhttps://linear.app/tezos/issue/JSTZ-902/handle-inbox-monitors-block-reorgs
                        error!("[handle_loading_checkpoint] Checkpoint duplicate/rewind, entering wait");
                        (State::Backoff, None)
                    }
                };
                self.fut.take();
                match next_level {
                    Some(level) => (
                        next_state,
                        Some(Poll::Ready(Some(Ok(PendingBlock::new(
                            self.store.clone(),
                            level,
                        ))))),
                    ),
                    None => (next_state, None),
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use anyhow::anyhow;
    use futures_util::future::BoxFuture;
    use futures_util::{pin_mut, stream};
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    pub(crate) struct MockStore {
        buffer: Arc<Mutex<Vec<BlockLevel>>>,
        store_error_count: Arc<AtomicUsize>,
    }

    impl MockStore {
        pub(crate) fn new() -> Self {
            Self {
                buffer: Arc::new(Mutex::new(vec![])),
                store_error_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn buffer(&self) -> Vec<BlockLevel> {
            self.buffer.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl CheckpointStore for MockStore {
        // Returns the last block after 10 blocks, then returns 3 errors before returning the last block.
        async fn load(&self) -> io::Result<Option<BlockLevel>> {
            let val = self.buffer.lock().unwrap().last().copied();
            if self.buffer.lock().unwrap().len() == 10 {
                if self.store_error_count.load(Ordering::SeqCst) == 3 {
                    return Ok(val);
                }
                self.store_error_count.fetch_add(1, Ordering::SeqCst);
                return Err(io::Error::other("storage error"));
            }
            Ok(val)
        }
        async fn save(&mut self, level: BlockLevel) -> io::Result<()> {
            self.buffer.lock().unwrap().push(level);
            Ok(())
        }
    }

    fn mock_stream(block_levels: Vec<anyhow::Result<BlockLevel>>) -> SourceStream {
        stream::iter(block_levels).boxed()
    }

    fn mock_store() -> MockStore {
        MockStore::new()
    }

    // Process the block stream and save the checkpoints to the store.
    fn process_stream<'a>(
        store: &'a mut MockStore,
        stream: SequentialBlockStream<MockStore, impl StreamFactory>,
    ) -> BoxFuture<'a, ()> {
        async {
            pin_mut!(stream);
            while let Some(pending_block) = stream.next().await {
                if let Ok(pending_block) = pending_block {
                    let _ = store.save(pending_block.level()).await;
                }
            }
        }
        .boxed()
    }

    #[tokio::test]
    async fn stream_follows_source_stream() {
        // Cold start: follows the source stream
        let mut store = mock_store();
        let factory = move || mock_stream(vec![1, 2, 3].into_iter().map(Ok).collect());
        let stream = SequentialBlockStream::new(store.clone(), factory);
        let future = process_stream(&mut store, stream);
        let result = tokio::time::timeout(Duration::from_secs(1), future).await;
        assert!(
            result.is_err(),
            "should timeout as the stream never ends and keeps retrying"
        );
        assert_eq!(store.buffer(), (1..=3).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn stream_handles_checkpoint_error() {
        let mut store = mock_store();
        // The store returns an error after 10 blocks, then returns the last block after 3 errors.
        let factory = move || {
            mock_stream(
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
                    .into_iter()
                    .map(Ok)
                    .collect(),
            )
        };
        let stream = SequentialBlockStream::new(store.clone(), factory);
        let handle = process_stream(&mut store, stream);
        let result = tokio::time::timeout(Duration::from_secs(3), handle).await;
        assert!(
            result.is_err(),
            "should timeout as the stream never ends and keeps retrying"
        );
        assert_eq!(store.buffer(), (1..=12).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn stream_fills_gap_if_source_stream_disconnects_and_reconnects() {
        let mut store = mock_store();
        let increment = |c: &AtomicUsize| c.fetch_add(1, Ordering::SeqCst);
        let count = AtomicUsize::new(0);
        let factory = move || match count.load(Ordering::SeqCst) {
            // First try, the source stream yields 1 and errors -> backoff
            0 => {
                increment(&count);
                mock_stream(
                    vec![Ok(1), Err(anyhow!("stream disconnected"))]
                        .into_iter()
                        .collect(),
                )
            }
            // Second try, consecutive errors from the source stream -> backoff
            1 => {
                increment(&count);
                mock_stream(
                    vec![Err(anyhow!("stream disconnected"))]
                        .into_iter()
                        .collect(),
                )
            }
            // Third try, the source stream yields 5 and fails -> ensures backlog is exhausted to fill the gap and backoff
            2 => {
                increment(&count);
                mock_stream(
                    vec![Ok(5), Err(anyhow!("stream disconnected"))]
                        .into_iter()
                        .collect(),
                )
            }
            // Fourth try, the source stream is reconnected quickly and still yields 5 -> backoff
            3 => {
                increment(&count);
                mock_stream(vec![Ok(5)].into_iter().collect())
            }
            // Fifth try, the source stream continues to yield blocks
            4 => mock_stream(vec![Ok(6), Ok(7)].into_iter().collect()),
            _ => unreachable!(),
        };
        let stream = SequentialBlockStream::new(store.clone(), factory);
        let handle = process_stream(&mut store, stream);
        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_err(),
            "should timeout as the stream never ends and keeps retrying"
        );
        assert_eq!(store.buffer(), (1..=7).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn stream_respects_backoff_delay() {
        let mut store = mock_store();
        let factory =
            move || mock_stream(vec![Err(anyhow!("stream error"))].into_iter().collect());

        let increment = |c: &AtomicUsize| c.fetch_add(1, Ordering::SeqCst);
        let count = Arc::new(AtomicUsize::new(0));
        let timestamps = Arc::new(Mutex::new(Vec::new()));
        let t = timestamps.clone();
        let count2 = count.clone();
        let factory = move || {
            let n = increment(&count2);
            if n > 0 {
                t.lock().unwrap().push(Instant::now());
            }
            match n {
                0..=2 => mock_stream(
                    vec![Err(anyhow!("stream disconnected"))]
                        .into_iter()
                        .collect(),
                ),
                3 => mock_stream(vec![Ok(1)].into_iter().collect()),
                _ => mock_stream(vec![]),
            }
        };
        let stream = SequentialBlockStream::new(store.clone(), factory);
        let handle = process_stream(&mut store, stream);
        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_err(),
            "should timeout as the stream never ends and keeps retrying"
        );
        assert_eq!(store.buffer(), vec![1]);
        let timestamps = timestamps.lock().unwrap();
        let first = *timestamps.first().unwrap();
        let second = *timestamps.get(1).unwrap();
        let third = *timestamps.get(2).unwrap();
        assert!(second.duration_since(first) >= Duration::from_millis(RETRY_MS));
        assert!(third.duration_since(second) >= Duration::from_millis(RETRY_MS));
    }
}
