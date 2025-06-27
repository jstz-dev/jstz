use std::path::PathBuf;

use futures_util::StreamExt;
use tokio::sync::broadcast;
use tokio::task::AbortHandle;

use crate::filtered_log_stream::FilteredLogStream;
use crate::request::{request_event_from_log_line, OracleRequest, ORACLE_LINE_REGEX};

use anyhow::Result;

/// A relay that forwards oracle requests from a log file to a channel.
pub struct Relay {
    tx: broadcast::Sender<OracleRequest>,
    subscriber_count: std::sync::atomic::AtomicUsize,
    abort_handle: AbortHandle,
}

impl Relay {
    pub async fn spawn(log_path: PathBuf) -> Result<Self> {
        let (tx, _rx0) = broadcast::channel(1024);

        let mut stream =
            FilteredLogStream::new(ORACLE_LINE_REGEX.clone(), log_path).await?;

        let abort_handle = {
            let task = tokio::spawn({
                let tx = tx.clone();
                async move {
                    while let Some(line_res) = stream.next().await {
                        match line_res {
                            Ok(line) => match request_event_from_log_line(&line) {
                                Ok(ev) => {
                                    if let Err(e) = tx.send(ev) {
                                        eprintln!("Failed to send event: {}", e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Failed to parse oracle log line: {}; line={}",
                                        e, line
                                    );
                                    break;
                                }
                            },
                            Err(e) => {
                                eprintln!("Log stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
            });
            task.abort_handle()
        };

        Ok(Self {
            tx,
            subscriber_count: std::sync::atomic::AtomicUsize::new(0),
            abort_handle,
        })
    }

    pub fn subscribe(&self) -> Result<broadcast::Receiver<OracleRequest>> {
        let current_count = self
            .subscriber_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if current_count > 0 {
            return Err(anyhow::anyhow!("Relay already has a subscriber"));
        }
        Ok(self.tx.subscribe())
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{io::Write, time::Duration};
    use tempfile::NamedTempFile;
    use tokio::{
        fs::OpenOptions,
        io::AsyncWriteExt,
        time::{sleep, timeout},
    };

    async fn append_async(path: PathBuf, line: String, delay_ms: u64) -> Result<()> {
        sleep(Duration::from_millis(delay_ms)).await;
        let mut file = OpenOptions::new().append(true).open(&path).await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
        Ok(())
    }

    async fn next_event(
        rx: &mut broadcast::Receiver<OracleRequest>,
    ) -> Result<OracleRequest> {
        timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout")
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn make_line(id: u64) -> String {
        format!(
            r#"[ORACLE]{{"id":{id},"caller":"tz1XSYefkGnDLgkUPUmda57jk1QD6kqk2VDb","gas_limit":100,"timeout":21,"request":{{"method":[80,79,83,84],"url":"http://example.com/foo","headers":[],"body":{{"Vector":[123,34,109,101,115,115,97,103,101,34,58,34,104,101,108,108,111,34,125]}}}}}}"#
        )
    }

    #[tokio::test]
    async fn forwards_single_valid_line() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let relay = Relay::spawn(path.clone()).await?;
        let mut rx = relay.subscribe()?;

        let id = 42;
        tokio::spawn(append_async(path, make_line(id), 25));

        let ev = next_event(&mut rx).await?;
        assert_eq!(ev.id, id);

        Ok(())
    }

    #[tokio::test]
    async fn ignores_noise_lines() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let relay = Relay::spawn(path.clone()).await?;
        let mut rx = relay.subscribe()?;

        let valid_id = 7;
        let path_clone = path.clone();
        tokio::spawn(async move {
            append_async(path_clone.clone(), "noise line".into(), 10).await?;
            append_async(path_clone, make_line(valid_id), 50).await
        });

        let ev = next_event(&mut rx).await?;
        assert_eq!(ev.id, valid_id);
        Ok(())
    }

    #[tokio::test]
    async fn ignores_preexisting_lines() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        writeln!(tmp, "{}", make_line(1))?;
        tmp.as_file_mut().sync_all()?;

        let path = tmp.path().to_path_buf();
        let relay = Relay::spawn(path.clone()).await?;
        let mut rx = relay.subscribe()?;

        let late_id = 2;
        tokio::spawn(append_async(path, make_line(late_id), 20));

        let ev = next_event(&mut rx).await?;
        assert_eq!(ev.id, late_id);
        Ok(())
    }

    #[tokio::test]
    async fn broadcasts_to_multiple_subscribers() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();

        let relay = Relay::spawn(path.clone()).await?;
        let mut rx1 = relay.subscribe()?;

        // Second subscription should fail
        let rx2_result = relay.subscribe();
        assert!(rx2_result.is_err());

        tokio::spawn(append_async(path, make_line(99), 10));

        let ev1 = next_event(&mut rx1).await?;
        assert_eq!(ev1.id, 99);
        Ok(())
    }
}
