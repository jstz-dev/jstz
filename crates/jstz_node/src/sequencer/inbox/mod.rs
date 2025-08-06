#![allow(unused_variables)]
#![allow(unreachable_code)]
use std::sync::{Arc, RwLock};

use crate::sequencer::queue::OperationQueue;
use crate::sequencer::runtime::{JSTZ_ROLLUP_ADDRESS, TICKETER};
use anyhow::Result;
use api::BlockResponse;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use jstz_core::host::WriteDebug;
use jstz_kernel::inbox::{parse_inbox_message_hex, ParsedInboxMessage};
use log::{debug, error};
use std::future::Future;
use std::time::Duration;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tokio::{select, task::JoinHandle};
use tokio_retry2::strategy::ExponentialFactorBackoff;
use tokio_retry2::{Retry, RetryError};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

pub mod api;

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

struct Logger;
impl WriteDebug for Logger {
    fn write_debug(&self, msg: &str) {
        debug!("{msg}");
    }
}

/// Spawn a future that monitors the L1 blocks, parses inbox messages and pushes them into the queue.
/// precondition: the rollup node is healthy.
pub async fn spawn_monitor<
    #[cfg(test)] Fut: Future<Output = ()> + 'static + Send,
    #[cfg(test)] F: Fn(u32) -> Fut + Send + 'static,
>(
    rollup_endpoint: String,
    queue: Arc<RwLock<OperationQueue>>,
    #[cfg(test)] on_new_block: F,
) -> Result<Monitor> {
    let kill_sig = CancellationToken::new();
    let kill_sig_clone = kill_sig.clone();
    let mut block_stream = api::monitor_blocks(&rollup_endpoint).await?;
    let handle = tokio::spawn(async move {
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        loop {
            select! {
                _ = kill_sig_clone.cancelled() => {
                    break;
                }
                result = block_stream.next() => {
                    match result {
                        Some(Ok(block)) => {
                            #[cfg(test)]
                            {
                                on_new_block(block.level).await;
                                continue;
                            }
                            let block_content = retry_fetch_block(&rollup_endpoint, block.level).await;
                            process_inbox_messages(block_content, queue.clone(), &ticketer, &jstz).await;
                        }
                        _ => {
                            //TODO: handle the case when the stream ended/errored
                            // https://linear.app/tezos/issue/JSTZ-622/handle-retrial-when-stream-connection-is-lost
                            error!("`monitor_blocks` stream connection lost");
                            break;
                        }
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

/// Process the inbox messages of the given block and push them into the queue.
async fn process_inbox_messages(
    block_content: BlockResponse,
    queue: Arc<RwLock<OperationQueue>>,
    ticketer: &ContractKt1Hash,
    jstz: &SmartRollupHash,
) {
    let mut ops = parse_inbox_messages(block_content, ticketer, jstz);
    while let Some(op) = ops.pop() {
        loop {
            let success = queue.write().is_ok_and(|mut q| q.insert_ref(&op).is_ok());
            if success {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// parse the inbox messages into jstz operations of the given block
fn parse_inbox_messages(
    block: BlockResponse,
    ticketer: &ContractKt1Hash,
    jstz: &SmartRollupHash,
) -> Vec<ParsedInboxMessage> {
    block
        .messages
        .iter()
        .enumerate()
        .filter_map(|(inbox_id, inbox_msg)| {
            parse_inbox_message_hex(&Logger, inbox_id as u32, inbox_msg, ticketer, jstz)
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
async fn retry_fetch_block(rollup_endpoint: &str, block_level: u32) -> BlockResponse {
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
    use crate::sequencer::inbox::test_utils::{hash_of, make_mock_monitor_blocks_filter};
    use jstz_kernel::inbox::LevelInfo;
    use jstz_kernel::inbox::Message;
    use std::time::Duration;
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex, RwLock},
    };
    use tokio::task;
    use tokio::time::{sleep, Instant};
    use tokio_retry2::RetryError;

    fn mock_deploy_op() -> (&'static str, &'static str) {
        let op = "0100c3ea4c18195bcfac262dcb29e3d803ae746817390000000040000000000000002c33da9518a6fce4c22a7ba352580d9097cacc9123df767adb40871cef49cbc7efebffcb4a1021b514dca58450ac9c50e221deaeb0ed2034dd36f1ae2de11f0f00000000200000000000000073c58fbff04bb1bc965986ad626d2a233e630ea253d49e1714a0bc9610c1ef450000000000000000000000000901000000000000636f6e7374204b4559203d2022636f756e746572223b0a0a636f6e73742068616e646c6572203d202829203d3e207b0a20206c657420636f756e746572203d204b762e676574284b4559293b0a2020636f6e736f6c652e6c6f672860436f756e7465723a20247b636f756e7465727d60293b0a202069662028636f756e746572203d3d3d206e756c6c29207b0a20202020636f756e746572203d20303b0a20207d20656c7365207b0a20202020636f756e7465722b2b3b0a20207d0a20204b762e736574284b45592c20636f756e746572293b0a202072657475726e206e657720526573706f6e736528293b0a7d3b0a0a6578706f72742064656661756c742068616e646c65723b0a0000000000000000";
        let op_hash = "eea5a17541e509914c7ebe48dd862ba5b96b878522a01132fc881080278a6b83";
        (op_hash, op)
    }

    type OnNewBlockCallback =
        Box<dyn Fn(u32) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static>;

    fn make_on_new_block() -> (Arc<Mutex<u32>>, OnNewBlockCallback) {
        let counter = Arc::new(Mutex::new(0u32));
        let counter_clone = counter.clone();
        let on_new_block = move |num: u32| {
            let counter_clone = counter_clone.clone();
            Box::pin(async move {
                let mut value = counter_clone.lock().unwrap();
                *value = num;
            })
        }
            as Pin<Box<dyn Future<Output = ()> + Send>>;
        (counter, Box::new(on_new_block))
    }

    #[tokio::test]
    async fn test_spawn_shuts_down() {
        let (addr, server) = warp::serve(make_mock_monitor_blocks_filter())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        task::spawn(server);
        let endpoint = format!("http://{addr}");
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (counter, on_new_block) = make_on_new_block();
        let mut monitor = spawn_monitor(endpoint.clone(), q.clone(), on_new_block)
            .await
            .unwrap();
        sleep(Duration::from_millis(100)).await;
        assert_eq!(*counter.lock().unwrap(), 123);
        monitor.shut_down().await;
        sleep(Duration::from_millis(400)).await;
        assert_eq!(*counter.lock().unwrap(), 123);
    }

    #[tokio::test]
    async fn test_spawn() {
        let (addr, server) = warp::serve(make_mock_monitor_blocks_filter())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        task::spawn(server);
        let endpoint = format!("http://{addr}");
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (counter, on_new_block) = make_on_new_block();
        let _ = spawn_monitor(endpoint, q, on_new_block).await.unwrap();
        sleep(Duration::from_millis(100)).await;
        assert_eq!(*counter.lock().unwrap(), 123);
        sleep(Duration::from_millis(400)).await;
        assert_eq!(*counter.lock().unwrap(), 124);
    }

    #[tokio::test]
    async fn test_parse_inbox_messages() {
        let (op_hash, op) = mock_deploy_op();
        let q = Arc::new(RwLock::new(OperationQueue::new(2)));
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let block_content = BlockResponse {
            messages: vec![String::from("0001"), String::from(op)],
        };
        let msgs = parse_inbox_messages(block_content, &ticketer, &jstz);
        assert_eq!(msgs.len(), 2);
        assert!(matches!(
            &msgs[0],
            ParsedInboxMessage::LevelInfo(LevelInfo::Start)
        ));
        assert!(
            matches!(&msgs[1], ParsedInboxMessage::JstzMessage(Message::External(op)) if op.hash().to_string() == op_hash)
        );
    }

    #[tokio::test]
    async fn test_process_inbox_messages() {
        let (op_hash, op) = mock_deploy_op();
        let q = Arc::new(RwLock::new(OperationQueue::new(2)));
        let ticketer = ContractKt1Hash::from_base58_check(TICKETER).unwrap();
        let jstz = SmartRollupHash::from_base58_check(JSTZ_ROLLUP_ADDRESS).unwrap();
        let block_content = BlockResponse {
            messages: vec![
                String::from("0000"),
                String::from(op),
                String::from(op),
                String::from("0001"),
            ],
        };

        let queue = q.clone();
        let handle = tokio::spawn(async move {
            let ticketer = ticketer;
            let jstz = jstz;
            process_inbox_messages(block_content, queue.clone(), &ticketer, &jstz).await;
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        // only one message should be in the queue to respect the limit
        assert_eq!(q.read().unwrap().len(), 2);

        let sol = q.write().unwrap().pop().unwrap();
        let op = q.write().unwrap().pop().unwrap();
        assert!(matches!(
            sol,
            ParsedInboxMessage::LevelInfo(LevelInfo::Start)
        ));
        assert_eq!(hash_of(&op), op_hash);
        assert_eq!(q.read().unwrap().len(), 0);

        // the waiting operation should be added to the queue now that the previous one is processed
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(q.read().unwrap().len(), 1);
        let op = q.write().unwrap().pop().unwrap();
        assert_eq!(hash_of(&op), op_hash);

        handle.abort();
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
    use bytes::Bytes;
    use futures_util::stream;
    use jstz_kernel::inbox::Message;
    use std::{convert::Infallible, time::Duration};
    use tokio::time::sleep;
    use warp::{hyper::Body, Filter};

    pub(crate) fn hash_of(op: &ParsedInboxMessage) -> String {
        match op {
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
            let response = BlockResponse {
                messages: vec![format!("message for block {}", level)],
            };
            warp::reply::json(&response)
        })
    }
}
