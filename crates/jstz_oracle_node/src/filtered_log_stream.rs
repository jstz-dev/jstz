use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use regex::Regex;
use tokio::sync::mpsc;

use jstz_utils::tailed_file::TailedFile;
use std::time::Duration;

pub struct FilteredLogStream {
    rx: mpsc::Receiver<Result<String>>,
}

impl FilteredLogStream {
    pub async fn new(pattern: Regex, path: PathBuf) -> Result<Self> {
        let file = TailedFile::init(&path).await?;

        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            let mut lines = file.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if pattern.is_match(&line.to_string())
                            && tx.send(Ok(line)).await.is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into())).await;
                        break;
                    }
                }
            }
        });

        Ok(Self { rx })
    }
}

impl futures_core::Stream for FilteredLogStream {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use std::{io::Write, time::Duration};
    use tempfile::NamedTempFile;
    use tokio::{fs::OpenOptions, io::AsyncWriteExt, time::timeout};

    const PATTERN: &str = r#"^\[ORACLE\]\s+\d+\s+.*"#;

    fn append_sync(file: &mut NamedTempFile, line: &str) -> anyhow::Result<()> {
        writeln!(file.as_file_mut(), "{line}")?;
        file.as_file_mut().sync_all()?;
        Ok(())
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

    fn make_line(id: u64, body: &str) -> String {
        format!("[ORACLE] {id} {body}")
    }

    async fn next_line(s: &mut FilteredLogStream, dur: Duration) -> Result<String> {
        timeout(dur, s.next()).await?.expect("stream ended")
    }

    #[tokio::test]
    async fn picks_up_new_line() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let stream =
            FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone()).await?;
        futures_util::pin_mut!(stream);

        let line = make_line(42, "foo");
        let writer = tokio::spawn(append_async(path, line.clone(), 25));

        assert_eq!(
            next_line(&mut stream, Duration::from_secs(1))
                .await?
                .trim_end(),
            line
        );

        writer.await??;
        Ok(())
    }

    #[tokio::test]
    async fn ignores_noise() -> anyhow::Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let stream =
            FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone()).await?;
        futures_util::pin_mut!(stream);

        let line = make_line(7, "bar");
        let writer = tokio::spawn(async move {
            append_async(path.clone(), "invalid line".into(), 10).await?;
            append_async(path, line.clone(), 100).await
        });

        assert_eq!(
            next_line(&mut stream, Duration::from_secs(1))
                .await?
                .trim_end(),
            make_line(7, "bar")
        );

        writer.await??;
        Ok(())
    }

    #[tokio::test]
    async fn skips_preexisting() -> anyhow::Result<()> {
        let mut tmp = NamedTempFile::new()?;
        append_sync(&mut tmp, &make_line(1, "early"))?;

        let path = tmp.path().to_path_buf();
        let stream =
            FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone()).await?;
        futures_util::pin_mut!(stream);

        let late = make_line(2, "late");
        let writer = tokio::spawn(append_async(path, late.clone(), 20));

        assert_eq!(
            next_line(&mut stream, Duration::from_secs(1))
                .await?
                .trim_end(),
            late
        );

        writer.await??;
        Ok(())
    }
}
