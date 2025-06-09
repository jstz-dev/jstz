#![allow(unused_variables)]
#![allow(unreachable_code)]
use std::sync::Arc;

use std::time::Duration;

use crate::sequencer::queue::OperationQueue;
use crate::sequencer::runtime::{JSTZ_ROLLUP_ADDRESS, TICKETER};
use anyhow::Result;
use api::BlockResponse;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use jstz_core::host::WriteDebug;
use log::{debug, error};
use parsing::{parse_inbox_message_hex, Message};
#[cfg(test)]
use std::future::Future;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};
use tokio::sync::RwLock;
use tokio::{select, task::JoinHandle};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

pub mod api;
mod parsing;

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
pub async fn spawn_monitor<
    #[cfg(test)] Fut: Future<Output = ()> + 'static + Send,
    #[cfg(test)] F: Fn(u32) -> Fut + Send + 'static,
>(
    rollup_endpoint: String,
    queue: Arc<RwLock<OperationQueue>>,
    #[cfg(test)] on_new_block: F,
) -> Result<Monitor> {
    // temp fix for jstzd to run locally.
    // TODO: add logic to wait until rollup node is `healthy` in jstzd
    #[cfg(not(test))]
    tokio::time::sleep(Duration::from_secs(3)).await;
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
        match op {
            Message::External(op) => {
                loop {
                    let success = queue.write().await.insert_ref(&op).is_ok();
                    if success {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Message::Internal(_) => {
                // TODO: handle internal messages (deposits)
                // https://linear.app/tezos/issue/JSTZ-637/handle-deposit-operation
                panic!("Internal messages not supported yet");
            }
        }
    }
}

/// parse the inbox messages into jstz operations of the given block
fn parse_inbox_messages(
    block: BlockResponse,
    ticketer: &ContractKt1Hash,
    jstz: &SmartRollupHash,
) -> Vec<Message> {
    block
        .messages
        .iter()
        .enumerate()
        .filter_map(|(inbox_id, inbox_msg)| {
            parse_inbox_message_hex(&Logger, inbox_id as u32, inbox_msg, ticketer, jstz)
        })
        .collect()
}

// Retry fetching the block indefinitely because:
// 1. We cannot progress without successfully fetching the block data
// 2. The block data must eventually become available (it's part of the chain)
// 3. Temporary network issues or API unavailability should not stop the sequencer
// 4. The exponential backoff ensures we don't overwhelm the API
async fn retry_fetch_block(rollup_endpoint: &str, block_level: u32) -> BlockResponse {
    let mut attempts = 0;
    let mut backoff = Duration::from_millis(200);
    const MAX_BACKOFF: Duration = Duration::from_secs(5);
    loop {
        match api::fetch_block(rollup_endpoint, block_level).await {
            Ok(block) => return block,
            Err(e) => {
                attempts += 1;
                error!(
                    "Retry {}: Failed to fetch block {}: {:?}",
                    attempts, block_level, e
                );
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::inbox::test_utils::make_mock_monitor_blocks_filter;
    use std::time::Duration;
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };
    use tokio::task;
    use tokio::time::sleep;

    fn make_on_new_block() -> (
        Arc<Mutex<u32>>,
        impl Fn(u32) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
    ) {
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
        (counter, on_new_block)
    }

    #[tokio::test]
    async fn test_spawn_shuts_down() {
        let (addr, server) = warp::serve(make_mock_monitor_blocks_filter())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        task::spawn(server);
        let endpoint = format!("http://{}", addr);
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
        let endpoint = format!("http://{}", addr);
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (counter, on_new_block) = make_on_new_block();
        let _ = spawn_monitor(endpoint, q, on_new_block).await.unwrap();
        sleep(Duration::from_millis(100)).await;
        assert_eq!(*counter.lock().unwrap(), 123);
        sleep(Duration::from_millis(400)).await;
        assert_eq!(*counter.lock().unwrap(), 124);
    }
}

#[cfg(test)]
mod test_utils {
    use super::{api::BlockResponse, *};
    use bytes::Bytes;
    use futures_util::stream;
    use std::{convert::Infallible, time::Duration};
    use tokio::time::sleep;
    use warp::{hyper::Body, Filter};

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
