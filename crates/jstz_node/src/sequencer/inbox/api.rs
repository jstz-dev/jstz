#![allow(dead_code)]
use anyhow::Result;
use futures_util::{Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;

/// Response structure for block data containing inbox messages.
/// Each message in the inbox is represented as a hex-encoded string.
#[derive(Debug, Deserialize, Serialize)]
pub struct BlockResponse {
    pub messages: Vec<String>,
}

/// Fetches block data from the rollup node for a specific block level.
pub async fn fetch_block(
    rollup_endpoint: &str,
    block_level: u32,
) -> Result<BlockResponse> {
    let url = format!("{}/global/block/{}", rollup_endpoint, block_level);
    let response = reqwest::get(url).await?;
    let block: BlockResponse = response.json().await?;
    Ok(block)
}

/// Response structure for block monitoring, containing the block level information.
#[derive(Debug, Deserialize)]
pub struct MonitorBlocksResponse {
    pub level: u32,
}

/// Establishes a streaming connection to monitor new blocks from the rollup node.
/// This function creates a long-lived connection that will receive updates whenever
/// new blocks are produced by the rollup node.
///
/// # Note
/// The connection uses TCP keepalive to maintain the connection and will automatically
/// attempt to reconnect if the connection is lost.
pub async fn monitor_blocks(
    rollup_endpoint: &str,
) -> Result<impl Stream<Item = Result<MonitorBlocksResponse, anyhow::Error>> + Unpin> {
    let url = format!("{}/global/monitor_blocks", rollup_endpoint);
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(Duration::from_secs(10)))
        .build()?;
    let response = client.get(url).send().await?;
    let bytes_stream = response.bytes_stream();
    let reader = StreamReader::new(
        bytes_stream.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
    );
    let line_stream = FramedRead::new(reader, LinesCodec::new());
    let response_stream = line_stream.map(|result| match result {
        Ok(line) => Ok(serde_json::from_str::<MonitorBlocksResponse>(&line).unwrap()),
        Err(e) => Err(e.into()),
    });
    Ok(response_stream)
}

#[cfg(test)]
mod tests {
    use crate::sequencer::inbox::{
        api::{fetch_block, monitor_blocks},
        test_utils::{make_mock_global_block_filter, make_mock_monitor_blocks_filter},
    };
    use tokio::task;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_monitor_blocks() {
        let (addr, server) = warp::serve(make_mock_monitor_blocks_filter())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        task::spawn(server);
        let endpoint = format!("http://{}", addr);

        let mut stream = monitor_blocks(&endpoint).await.unwrap();

        // Read and parse first block
        let block = stream.next().await.unwrap().unwrap();
        assert_eq!(block.level, 123);

        // Read and parse second block
        let block = stream.next().await.unwrap().unwrap();
        assert_eq!(block.level, 124);
    }

    #[tokio::test]
    async fn test_global_block() {
        let (addr, server) = warp::serve(make_mock_global_block_filter())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        task::spawn(server);
        let endpoint = format!("http://{}", addr);

        // Test block endpoint
        let block_response = fetch_block(&endpoint, 123).await.unwrap();
        assert_eq!(block_response.messages, vec!["message for block 123"]);
    }
}
