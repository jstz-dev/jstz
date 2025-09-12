#![allow(unused_variables)]
#![allow(unreachable_code)]
use crate::sequencer::inbox::store::{CheckpointStore, FileCheckpointStore};
use crate::sequencer::inbox::stream::{Error, PendingBlock, SequentialBlockStream};
use crate::sequencer::queue::{OperationQueue, WrappedOperation};
use crate::sequencer::runtime::{JSTZ_ROLLUP_ADDRESS, TICKETER};
use anyhow::Result;
use api::BlockResponse;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};
use jstz_core::host::WriteDebug;
use jstz_kernel::inbox::parse_inbox_message_hex;
use jstz_proto::operation::internal::InboxId;
use jstz_proto::BlockLevel;
use log::{debug, error};
use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tokio::{select, task::JoinHandle};
use tokio_retry2::strategy::ExponentialFactorBackoff;
use tokio_retry2::{Retry, RetryError};
use tokio_util::sync::CancellationToken;

pub mod api;
pub mod store;
pub mod stream;

#[derive(Default)]
pub struct Monitor {
    inner: Option<JoinHandle<()>>,
    kill_sig: CancellationToken,
}

impl Monitor {
    pub async fn shut_down(&mut self) {
        self.kill_sig.cancel();
        if let Some(h) = self.inner.take() {
            let _ = h.await;
        }
    }
}

#[async_trait]
impl AsyncDrop for Monitor {
    async fn async_drop(&mut self) {
        self.shut_down().await;
    }
}

pub struct Logger;
impl WriteDebug for Logger {
    fn write_debug(&self, msg: &str) {
        debug!("{msg}");
    }
}

/// Spawn a future that monitors the L1 blocks, parses inbox messages and pushes them into the queue.
/// precondition: the rollup node is healthy.
pub async fn spawn_monitor(
    rollup_endpoint: String,
    queue: Arc<RwLock<OperationQueue>>,
    checkpoint_path: PathBuf,
) -> Result<Monitor> {
    let kill_sig = CancellationToken::new();
    let kill_sig_clone = kill_sig.clone();
    let store = FileCheckpointStore::new(checkpoint_path);
    let endpoint = rollup_endpoint.clone();
    let mut block_stream =
        SequentialBlockStream::new(store, move || stream_factory(endpoint.clone()))
            .boxed();
    let handle: JoinHandle<()> = tokio::spawn(async move {
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        loop {
            select! {
                _ = kill_sig_clone.cancelled() => {
                    break;
                }
                result = block_stream.next() => {
                    match result {
                        Some(Ok(mut block)) => {
                            let block_content = retry_fetch_block(&rollup_endpoint, block.level()).await;
                            process_inbox_messages(&mut block, block_content, queue.clone(), &ticketer, &jstz).await;
                        }
                        Some(Err(Error::CheckpointIo(e))) => {
                            error!("checkpoint io error: {e:?}");
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        None => unreachable!(),
                    }
                }
            }
        }
    });

    Ok(Monitor {
        inner: Some(handle),
        kill_sig,
    })
}

fn stream_factory(endpoint: String) -> BoxStream<'static, Result<BlockLevel>> {
    let stream = async move {
        api::monitor_blocks(&endpoint)
            .await
            .map(|s| s.map_ok(|b| b.level))
    };

    futures_util::stream::once(stream).try_flatten().boxed()
}

/// Processes inbox messages for the given block:
/// 1. Parses messages into operations.
/// 2. Pushes each operation into the shared queue, retrying on failure.
/// 3. Commits the block as a checkpoint after all operations are queued.
async fn process_inbox_messages<S: CheckpointStore>(
    block: &mut PendingBlock<S>,
    block_content: BlockResponse,
    queue: Arc<RwLock<OperationQueue>>,
    ticketer: &ContractKt1Hash,
    jstz: &SmartRollupHash,
) {
    let mut ops = parse_inbox_messages(block.level(), block_content, ticketer, jstz);
    let push =
        |op: &WrappedOperation| queue.write().is_ok_and(|mut q| q.insert_ref(op).is_ok());

    while let Some(op) = ops.pop_front() {
        let mut queued = push(&op);
        loop {
            let exit = match (queued, ops.is_empty()) {
                // Save as checkpoint once the last op is pushed to the queue
                (true, true) => block.commit().await.is_ok(),
                // continue on with the next op
                (true, false) => true,
                // Push the op to the queue and save as checkpoint
                (false, true) => {
                    queued = push(&op);
                    queued && block.commit().await.is_ok()
                }
                (false, false) => push(&op),
            };
            if exit {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// parse the inbox messages into jstz operations of the given block
fn parse_inbox_messages(
    block_level: BlockLevel,
    block_content: BlockResponse,
    ticketer: &ContractKt1Hash,
    jstz: &SmartRollupHash,
) -> VecDeque<WrappedOperation> {
    block_content
        .messages
        .iter()
        .enumerate()
        .filter_map(|(l1_message_id, inbox_msg)| {
            parse_inbox_message_hex(
                &Logger,
                InboxId {
                    l1_level: block_level as u32,
                    l1_message_id: l1_message_id as u32,
                },
                inbox_msg,
                ticketer,
                jstz,
            )
            .map(|op| WrappedOperation::FromInbox {
                message: op,
                original_inbox_message: inbox_msg.to_string(),
            })
        })
        .collect()
}

/// Retry the given async function using exponential backoff until it succeeds.
///
/// - Starts at `initial_delay_ms` milliseconds
/// - Backs off exponentially
/// - Max delay is capped at 5 seconds
pub(crate) async fn retry_expo<T, E, Fut>(
    initial_delay_ms: u64,
    f: impl FnMut() -> Fut,
) -> T
where
    Fut: Future<Output = Result<T, RetryError<E>>>,
{
    let backoff = ExponentialFactorBackoff::from_millis(initial_delay_ms, 2.0)
        .max_delay(Duration::from_secs(5));
    match Retry::spawn(backoff, f).await {
        Ok(val) => val,
        _ => unreachable!("Exponential backoff is infinite; this should never fail"),
    }
}

// Retry fetching the block indefinitely because:
// 1. We cannot progress without successfully fetching the block data
// 2. The block data must eventually become available (it's part of the chain)
// 3. Temporary network issues or API unavailability should not stop the sequencer
// 4. The exponential backoff ensures we don't overwhelm the API
async fn retry_fetch_block(
    rollup_endpoint: &str,
    block_level: BlockLevel,
) -> BlockResponse {
    retry_expo(200, || async {
        match api::fetch_block(rollup_endpoint, block_level).await {
            Ok(block) => Ok(block),
            Err(e) => {
                error!("Failed to fetch block {}: {:?}", block_level, e);
                Err(RetryError::transient(e))
            }
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::inbox::test_utils::make_mock_global_block_filter;
    use crate::sequencer::inbox::test_utils::{hash_of, make_mock_monitor_blocks_filter};
    use jstz_kernel::inbox::encode_signed_operation;
    use jstz_kernel::inbox::LevelInfo;
    use jstz_kernel::inbox::Message;
    use jstz_kernel::inbox::ParsedInboxMessage;
    use jstz_proto::operation::DeployFunction;
    use jstz_proto::operation::Operation;
    use jstz_proto::operation::SignedOperation;
    use jstz_proto::runtime::ParsedCode;
    use jstz_utils::test_util::alice_keys;
    use jstz_utils::KeyPair;
    use std::sync::{Arc, Mutex, RwLock};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tezos_smart_rollup::types::SmartRollupAddress;
    use tokio::task;
    use tokio::time::{sleep, Instant};
    use tokio_retry2::RetryError;
    use warp::Filter;

    pub fn mock_deploy_op(nonce: u64) -> SignedOperation {
        let KeyPair(alice_pk, alice_sk) = alice_keys();
        let code = r#"
            const handler = async () => {{
                return new Response();
            }};
            export default handler;
            "#;

        let deploy_fn = DeployFunction {
            function_code: ParsedCode::try_from(code.to_string()).unwrap(),
            account_credit: 0,
        };
        let op = Operation {
            public_key: alice_pk.clone(),
            nonce: nonce.into(),
            content: deploy_fn.into(),
        };
        SignedOperation::new(alice_sk.sign(op.hash()).unwrap(), op.clone())
    }

    // Returns the hex-encoded serialized external message for a given SignedOperation.
    pub fn hex_external_message(op: SignedOperation) -> String {
        let bytes = encode_signed_operation(
            &op,
            &SmartRollupAddress::from_b58check(JSTZ_ROLLUP_ADDRESS).unwrap(),
        )
        .unwrap();
        hex::encode(bytes)
    }

    fn spawn_mock_server() -> (String, JoinHandle<()>) {
        let filter =
            make_mock_monitor_blocks_filter().or(make_mock_global_block_filter());
        let (addr, server) = warp::serve(filter).bind_ephemeral(([127, 0, 0, 1], 0));
        (format!("http://{addr}"), task::spawn(server))
    }

    #[tokio::test]
    async fn test_spawn_shuts_down() {
        let (endpoint, _server) = spawn_mock_server();
        let q = Arc::new(RwLock::new(OperationQueue::new(10)));
        let file = NamedTempFile::new().unwrap();
        let store: FileCheckpointStore =
            FileCheckpointStore::new(file.path().to_path_buf());
        let mut monitor =
            spawn_monitor(endpoint.clone(), q.clone(), file.path().to_path_buf())
                .await
                .unwrap();
        sleep(Duration::from_millis(300)).await;
        assert_eq!(store.load().await.unwrap(), Some(123));
        monitor.shut_down().await;
        sleep(Duration::from_millis(400)).await;
        assert_eq!(store.load().await.unwrap(), Some(123));
    }

    #[tokio::test]
    async fn test_spawn() {
        let (endpoint, _server) = spawn_mock_server();
        let q = Arc::new(RwLock::new(OperationQueue::new(10)));
        let file = NamedTempFile::new().unwrap();
        let store = FileCheckpointStore::new(file.path().to_path_buf());
        let p = file.path().to_path_buf();
        let _monitor = spawn_monitor(endpoint, q, file.path().to_path_buf())
            .await
            .unwrap();
        while let Some(chk) = store.load().await.unwrap() {
            if chk == 123 {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        sleep(Duration::from_millis(400)).await;
        assert_eq!(store.load().await.unwrap(), Some(124));
    }

    #[tokio::test]
    async fn test_parse_inbox_messages() {
        let op = mock_deploy_op(0);
        let op_hash = op.hash().to_string();
        let q = Arc::new(RwLock::new(OperationQueue::new(2)));
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let raw_messages = vec![String::from("0001"), hex_external_message(op.clone())];
        let block_content = BlockResponse {
            messages: raw_messages.clone(),
        };
        let msgs = parse_inbox_messages(1, block_content, &ticketer, &jstz);
        assert_eq!(msgs.len(), 2);
        assert!(matches!(
        &msgs[0],
            WrappedOperation::FromInbox{
                message: ParsedInboxMessage::LevelInfo(LevelInfo::Start),
                original_inbox_message
            } if original_inbox_message == &raw_messages[0]
        ));
        assert!(matches!(&msgs[1], WrappedOperation::FromInbox{
                message: ParsedInboxMessage::JstzMessage(Message::External(op)),
                original_inbox_message } if op.hash().to_string() == op_hash
                && original_inbox_message == &raw_messages[1]));
    }

    #[tokio::test]
    async fn process_inbox_messages_respects_queue_size_and_order() {
        let op1 = mock_deploy_op(0);
        let op2 = mock_deploy_op(1);
        let q = Arc::new(RwLock::new(OperationQueue::new(2)));
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let block_content = BlockResponse {
            messages: vec![
                String::from("0001"), // Start of block
                hex_external_message(op1.clone()),
                hex_external_message(op2.clone()),
                String::from("FOO"), // Noise to be ignored
            ],
        };
        let store = stream::tests::MockStore::new();
        let mut block = PendingBlock::new(store.clone(), 1);
        let queue = q.clone();
        let handle = tokio::spawn(async move {
            process_inbox_messages(
                &mut block,
                block_content,
                queue.clone(),
                &ticketer,
                &jstz,
            )
            .await;
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        // only two messages should be in the queue to respect the queue limit
        assert_eq!(q.read().unwrap().len(), 2);
        let sol = q.write().unwrap().pop().unwrap();
        let op = q.write().unwrap().pop().unwrap();
        assert!(matches!(
            sol,
            WrappedOperation::FromInbox{
                message: ParsedInboxMessage::LevelInfo(LevelInfo::Start),
                original_inbox_message
            } if original_inbox_message == "0001"
        ));
        assert_eq!(hash_of(&op), op1.hash().to_string());
        assert_eq!(q.read().unwrap().len(), 0);

        // the waiting operation should be added to the queue now that the previous one is processed
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(q.read().unwrap().len(), 1);
        let op = q.write().unwrap().pop().unwrap();
        assert_eq!(hash_of(&op), op2.hash().to_string());
        // Checkpoint stores the block level 1
        assert_eq!(store.load().await.unwrap().unwrap(), 1);
        handle.abort();
    }

    #[tokio::test]
    async fn process_inbox_messages_retires_until_checkpoint_is_saved() {
        let block_level = 6;
        let messages = vec![mock_deploy_op(0), mock_deploy_op(1)]
            .iter()
            .map(|op| hex_external_message(op.clone()))
            .collect();
        let q = Arc::new(RwLock::new(OperationQueue::new(3)));
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let block_content = BlockResponse { messages };
        let mut store = stream::tests::MockStore::new();
        for i in 1..block_level {
            store.save(i).await.unwrap();
        }
        let mut block = PendingBlock::new(store.clone(), block_level);
        let queue = q.clone();
        let time = Instant::now();
        process_inbox_messages(
            &mut block,
            block_content,
            queue.clone(),
            &ticketer,
            &jstz,
        )
        .await;
        // The mock store fails twice before succeeding to save the checkpoint.
        // It should take at least 200ms to process the entire operations and save the checkpoint.
        assert!(time.elapsed() > Duration::from_millis(200));
        assert_eq!(q.read().unwrap().len(), 2);
        assert_eq!(store.load().await.unwrap().unwrap(), block_level);
    }

    #[tokio::test]
    async fn test_retry_expo_retries_and_succeeds() {
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = attempts.clone();
        let fail_times = 3;
        let time_before = Instant::now();
        let result = retry_expo(10, || {
            let attempts_clone = attempts_clone.clone();
            async move {
                let mut lock = attempts_clone.lock().unwrap();
                *lock += 1;
                if *lock <= fail_times {
                    Err(RetryError::transient("fail"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;
        let time_after = Instant::now();
        let duration = time_after.duration_since(time_before);
        // 10 + 20 + 40 = 70ms
        assert!(duration > Duration::from_millis(70));
        assert!(duration < Duration::from_millis(80));
        assert_eq!(result, "success");
        assert_eq!(*attempts.lock().unwrap(), fail_times + 1);
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use super::{api::BlockResponse, *};
    use crate::sequencer::inbox::tests::hex_external_message;
    use crate::sequencer::inbox::tests::mock_deploy_op;
    use bytes::Bytes;
    use futures_util::{stream, StreamExt};
    use jstz_kernel::inbox::Message;
    use jstz_kernel::inbox::ParsedInboxMessage;
    use std::{convert::Infallible, time::Duration};
    use tokio::time::sleep;
    use warp::{hyper::Body, Filter};

    pub(crate) fn hash_of(op: &WrappedOperation) -> String {
        let message = match op {
            WrappedOperation::FromInbox {
                message,
                original_inbox_message,
            } => message,
            WrappedOperation::FromNode(v) => return v.hash().to_string(),
        };
        match message {
            ParsedInboxMessage::JstzMessage(Message::External(op)) => {
                op.hash().to_string()
            }
            ParsedInboxMessage::JstzMessage(_) => {
                panic!("no hash for internal operation");
            }
            ParsedInboxMessage::LevelInfo(_) => {
                panic!("no hash for level info messages");
            }
        }
    }

    /// mock the /global/monitor_blocks endpoint
    pub(crate) fn make_mock_monitor_blocks_filter(
    ) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
    {
        warp::path!("global" / "monitor_blocks").map(|| {
            let delay_stream = stream::once(async {
                sleep(Duration::from_millis(300)).await;
                Ok::<Bytes, Infallible>(Bytes::new())
            });

            let data_stream = stream::iter(vec![Ok::<Bytes, Infallible>(Bytes::from(
                "{\"level\": 123}\n",
            ))])
            .chain(delay_stream)
            .chain(stream::iter(vec![Ok::<Bytes, Infallible>(Bytes::from(
                "{\"level\": 124}\n",
            ))]));
            warp::reply::Response::new(Body::wrap_stream(data_stream))
        })
    }

    /// mock the /global/block/[block_level] endpoint
    pub(crate) fn make_mock_global_block_filter(
    ) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
    {
        warp::path!("global" / "block" / u32).map(|level: u32| {
            let deploy_op1 = mock_deploy_op(1);
            let deploy_op2 = mock_deploy_op(2);
            let response = BlockResponse {
                messages: vec![
                    &hex_external_message(deploy_op1),
                    &hex_external_message(deploy_op2),
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            };
            warp::reply::json(&response)
        })
    }
}
