use crate::filtered_log_stream::FilteredLogStream;
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

fn filter_pattern<E: Event>() -> anyhow::Result<Regex> {
    Regex::new(&format!(r"^\[{}\].*$", E::tag())).map_err(anyhow::Error::from)
}

impl<'a, E: Event> EventStream<'a, E> {
    /// Create an [`EventStream`] from a kernel log file.
    ///
    /// Lines that match the prefix `E::tag()` will be decoded into event of type `E`. Otherwise, they will be ignored
    pub async fn from_file(path: PathBuf) -> Result<Self> {
        let pattern = filter_pattern::<E>()?;
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
            Poll::Ready(Some(Ok(line))) => match decode_line::<E>(&line) {
                Ok(event) => Poll::Ready(Some(Ok(event))),
                Err(e) => {
                    log::warn!("Failed to decode Event {}: {}", E::tag(), e);
                    Poll::Pending
                }
            },
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
    use std::{str::FromStr, time::Duration};
    use tempfile::NamedTempFile;
    use tokio::{fs::OpenOptions, io::AsyncWriteExt, time::timeout};
    use url::Url;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct MockEvent {
        pub id: u64,
        pub url: Url,
    }

    impl Event for MockEvent {
        fn tag() -> &'static str {
            "MOCK"
        }
    }

    fn mock_event(id: u64) -> MockEvent {
        MockEvent {
            id,
            url: Url::from_str("http://example.com/foo").unwrap(),
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

    #[tokio::test]
    async fn picks_up_new_event() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let mut stream = EventStream::from_file(path.clone()).await?;
        let req = mock_event(1);
        let writer = tokio::spawn(append_async(path, make_line(&req), 25));
        assert_eq!(next_event(&mut stream, Duration::from_secs(1)).await?, req);

        writer.await??;
        Ok(())
    }

    #[tokio::test]
    async fn ignores_noise() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let mut stream = EventStream::from_file(path.clone()).await?;
        let mock_event = mock_event(1);
        let line = make_line(&mock_event);
        let writer = tokio::spawn(async move {
            append_async(path.clone(), "invalid line".into(), 10).await?;
            append_async(path, line.clone(), 100).await
        });

        assert_eq!(
            next_event(&mut stream, Duration::from_secs(1)).await?,
            mock_event
        );

        writer.await??;
        Ok(())
    }

    #[test]
    fn filters_pattern() {
        let event = mock_event(42);
        let line = format!(
            "[{}]{}",
            MockEvent::tag(),
            serde_json::to_string(&event).unwrap()
        );
        let pattern = filter_pattern::<MockEvent>().unwrap();
        assert!(pattern.is_match(&line));

        let wrong_line = "[WRONG]: abc".to_string();
        assert!(!pattern.is_match(&wrong_line));
    }
}
