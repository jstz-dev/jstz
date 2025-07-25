use crate::tailed_file::FilteredLogStream;
use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use futures_core::{stream::BoxStream, Stream};
use jstz_core::event::{decode_line, Event};
use regex::Regex;
use std::path::PathBuf;
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A stream of logs.
pub type LogStream<'a> = BoxStream<'a, anyhow::Result<String>>;

/// A stream of Jstz Events, decoded from a log stream.
pub struct EventStream<'a, E: Event> {
    log_stream: LogStream<'a>,
    _marker: PhantomData<E>,
}

impl<'a, E: Event> EventStream<'a, E> {
    /// Create an event stream from a file.
    ///
    /// The file is expected to contain lines matching the given pattern.
    /// The pattern is a regular expression that will be used to filter the lines.
    /// The lines that match the pattern will be decoded into event of type `E`.
    /// The lines that do not match the pattern will be ignored.
    pub async fn from_file(pattern: Regex, path: PathBuf) -> Result<Self> {
        let stream = FilteredLogStream::new(pattern, path).await?;
        let stream = EventStream::<E> {
            log_stream: stream.map_err(anyhow::Error::from).boxed(),
            _marker: PhantomData,
        };
        Ok(stream)
    }
}

impl<'a, E: Event + Unpin> Stream for EventStream<'a, E> {
    type Item = anyhow::Result<E>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.log_stream.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(line))) => {
                match decode_line::<E>(&line) {
                    Ok(event) => Poll::Ready(Some(Ok(event))),
                    Err(_) => Poll::Pending, // Ignore noises.
                }
            }
            // An unrecoverable I/O error occurred while reading the file.
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    use serde::{Deserialize, Serialize};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tokio::{fs::OpenOptions, io::AsyncWriteExt, time::timeout};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct MockEvent {
        pub id: u64,
    }

    impl Event for MockEvent {
        fn tag() -> &'static str {
            "MOCK"
        }
    }

    async fn append_async(
        path: PathBuf,
        line: String,
        delay_ms: u64,
    ) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        let mut file = OpenOptions::new().append(true).open(&path).await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
        Ok(())
    }

    fn make_line(req: &MockEvent) -> String {
        format!("[MOCK] {}", serde_json::to_string(req).unwrap())
    }

    async fn next_event(
        s: &mut EventStream<'static, MockEvent>,
        dur: Duration,
    ) -> Result<MockEvent> {
        timeout(dur, s.next()).await?.expect("stream ended")
    }

    fn mock_request(id: u64) -> MockEvent {
        MockEvent { id }
    }

    fn regex() -> Regex {
        Regex::new(r#"^\[MOCK\]\s*(?P<json>\{.*\})\s*$"#).expect("hard-coded regex")
    }

    #[tokio::test]
    async fn picks_up_new_request() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let stream = EventStream::from_file(regex(), path.clone()).await?;
        futures_util::pin_mut!(stream);

        let req = mock_request(1);
        let writer = tokio::spawn(append_async(path, make_line(&req), 25));
        assert_eq!(next_event(&mut stream, Duration::from_secs(1)).await?, req);

        writer.await??;
        Ok(())
    }

    #[tokio::test]
    async fn ignores_noise() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let stream = EventStream::from_file(regex(), path.clone()).await?;
        futures_util::pin_mut!(stream);
        let mock_request = mock_request(1);
        let line = make_line(&mock_request);
        let writer = tokio::spawn(async move {
            append_async(path.clone(), "invalid line".into(), 10).await?;
            append_async(path, line.clone(), 100).await
        });

        assert_eq!(
            next_event(&mut stream, Duration::from_secs(1)).await?,
            mock_request
        );

        writer.await??;
        Ok(())
    }
}
