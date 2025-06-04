use std::{
    io,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::sequencer::queue::OperationQueue;
use anyhow::Result;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use serde::Deserialize;
#[cfg(test)]
use std::future::Future;
use tokio::{select, task::JoinHandle};
use tokio_stream::StreamExt;
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    io::StreamReader,
    sync::CancellationToken,
};

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

#[derive(Debug, Deserialize)]
struct MonitorBlocksResponse {
    level: u32,
}

/// Spawn a future that monitors the L1 blocks, parses inbox messages and pushes them into the queue.
pub async fn spawn_monitor<
    #[cfg(test)] Fut: Future<Output = ()> + 'static + Send,
    #[cfg(test)] F: Fn(u32) -> Fut + Send + 'static,
>(
    rollup_endpoint: String,
    _queue: Arc<RwLock<OperationQueue>>,
    #[cfg(test)] on_new_block: F,
) -> Result<Monitor> {
    // temp fix for jstzd to run locally.
    // TODO: add logic to wait until rollup node is `healthy` in jstzd
    #[cfg(not(test))]
    tokio::time::sleep(Duration::from_secs(3)).await;
    let kill_sig = CancellationToken::new();
    let kill_sig_clone = kill_sig.clone();

    let bytes_stream = monitor_blocks(&rollup_endpoint).await?;
    let reader = StreamReader::new(
        bytes_stream.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
    );
    let mut line_stream = FramedRead::new(reader, LinesCodec::new());
    let handle = tokio::spawn(async move {
        loop {
            select! {
                _ = kill_sig_clone.cancelled() => {
                    break;
                }
                result = line_stream.next() => {
                    match result {
                        Some(Ok(line)) => {
                            let block = serde_json::from_str::<MonitorBlocksResponse>(&line).unwrap();
                            //TODO: fetch inbox messages and place into the queue
                            println!("block level: {}\n", block.level);
                            #[cfg(test)]
                            on_new_block(block.level).await;
                        }
                        _ => {
                            //TODO: handle the case when the stream ended/errored
                            // https://linear.app/tezos/issue/JSTZ-622/handle-retrial-when-stream-connection-is-lost
                            println!("`monitor_blocks` stream connection lost");
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

/// Establishes a streaming connection to monitor new blocks from the rollup node.
/// Returns a stream of bytes that can be parsed into block information.
pub async fn monitor_blocks(
    rollup_endpoint: &str,
) -> Result<impl Stream<Item = Result<Bytes, reqwest::Error>> + Unpin> {
    let url = format!("{}/global/monitor_blocks", rollup_endpoint);
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(Duration::from_secs(10)))
        .build()?;
    let response = client.get(url).send().await?;
    Ok(response.bytes_stream())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::{convert::Infallible, time::Duration};
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex, RwLock},
    };
    use tokio::{task, time::sleep};
    use warp::{hyper::Body, Filter};

    struct MockServer(JoinHandle<()>);
    impl Drop for MockServer {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    /// mock the /global/monitor_blocks endpoint
    fn make_mock_monitor_blocks(
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

    fn make_mock_server() -> (String, MockServer) {
        let filter = make_mock_monitor_blocks();
        let (addr, server) = warp::serve(filter).bind_ephemeral(([127, 0, 0, 1], 0));
        let url = format!("http://{}", addr);
        (url, MockServer(task::spawn(server)))
    }

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
    async fn test_monitor_blocks() {
        let (endpoint, _server) = make_mock_server();

        let mut stream = monitor_blocks(&endpoint).await.unwrap();

        // Read and parse first block
        let bytes = stream.next().await.unwrap().unwrap();
        let line = String::from_utf8(bytes.to_vec()).unwrap();
        let block: MonitorBlocksResponse = serde_json::from_str(&line).unwrap();
        assert_eq!(block.level, 123);

        // Read and parse second block
        let bytes = stream.next().await.unwrap().unwrap();
        let line = String::from_utf8(bytes.to_vec()).unwrap();
        let block: MonitorBlocksResponse = serde_json::from_str(&line).unwrap();
        assert_eq!(block.level, 124);
    }

    #[tokio::test]
    async fn test_spawn_shuts_down() {
        let (endpoint, _server) = make_mock_server();
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
        let (endpoint, _server) = make_mock_server();
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (counter, on_new_block) = make_on_new_block();
        let _ = spawn_monitor(endpoint, q, on_new_block).await.unwrap();
        sleep(Duration::from_millis(100)).await;
        assert_eq!(*counter.lock().unwrap(), 123);
        sleep(Duration::from_millis(400)).await;
        assert_eq!(*counter.lock().unwrap(), 124);
    }
}
