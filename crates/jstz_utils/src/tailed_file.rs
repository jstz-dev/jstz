use std::{io::SeekFrom, path::Path};

use tokio::{
    fs::File,
    io::Lines,
    io::{AsyncSeekExt, BufReader, Result},
};

use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use regex::Regex;
use tokio::sync::mpsc;

use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct TailedFile(BufReader<File>);

pub use tokio::io::AsyncBufReadExt;

impl TailedFile {
    pub async fn init(path: &Path) -> Result<Self> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let _ = reader.seek(SeekFrom::End(0)).await?;
        Ok(TailedFile(reader))
    }

    pub fn lines(self) -> Lines<BufReader<File>> {
        self.0.lines()
    }
}

/// A stream that “tails” a file and yields only the lines that match the supplied regular expression.
///
/// * Opens the file in *follow* mode, emits only lines that are appended afterwards.
/// * Reads until the end of the file, then waits a bit and tries again.
///
/// Dropping `FilteredLogStream` calls `cancel()` and closes the file.
pub struct FilteredLogStream {
    rx: mpsc::Receiver<Result<String>>,
    cancel: CancellationToken,
}

impl FilteredLogStream {
    pub async fn new(pattern: Regex, path: PathBuf) -> anyhow::Result<Self> {
        let file = TailedFile::init(&path).await?;

        let (tx, rx) = mpsc::channel(1024);

        let cancel = CancellationToken::new();
        let token = cancel.clone();

        tokio::spawn(async move {
            let mut lines = file.lines();
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    line = lines.next_line() => match line {
                        Ok(Some(line)) => { // A new line matching `pattern` was appended.
                            if pattern.is_match(&line.to_string())
                                && tx.send(Ok(line)).await.is_err()
                            {
                                break;
                            }
                        }
                        Ok(None) => { // EOF – wait a bit and try again
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        Err(e) => { // An unrecoverable I/O error occurred while reading the file.
                            let _ = tx.send(Err(e)).await;
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self { rx, cancel })
    }
}

impl Drop for FilteredLogStream {
    fn drop(&mut self) {
        self.cancel.cancel();
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

    #[tokio::test]
    async fn cancels_on_drop() -> anyhow::Result<()> {
        use futures_util::FutureExt;

        let tmp = tempfile::NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let stream =
            FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone()).await?;
        let token = stream.cancel.clone();

        drop(stream);

        tokio::time::timeout(Duration::from_secs(1), token.cancelled().fuse())
            .await
            .expect(
                "background task did not terminate after FilteredLogStream was dropped",
            );

        Ok(())
    }
}
