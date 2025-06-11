use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use regex::Regex;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncSeekExt, BufReader},
    sync::mpsc,
};

pub struct FilteredLogStream {
    rx: mpsc::Receiver<Result<String>>,
}

impl FilteredLogStream {
    pub fn new(pattern: Regex, path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            let file = match File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(Err(e.into())).await;
                    return;
                }
            };
            let mut reader = BufReader::new(file);
            if let Err(e) = reader.seek(std::io::SeekFrom::End(0)).await {
                let _ = tx.send(Err(e.into())).await;
                return;
            }

            let mut buf = String::new();

            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Ok(_) => {
                        while buf.ends_with(['\n', '\r']) {
                            buf.pop();
                        }
                        if pattern.is_match(&buf) {
                            let _ = tx.send(Ok(buf.clone())).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into())).await;
                        return;
                    }
                }
            }
        });

        Self { rx }
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

        let stream = FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone());
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

        let stream = FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone());
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
        let stream = FilteredLogStream::new(Regex::new(PATTERN).unwrap(), path.clone());
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
