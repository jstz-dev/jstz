use futures_util::future::BoxFuture;
use jstz_proto::BlockLevel;
use serde::{Deserialize, Serialize};
use std::io::{self, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, io::AsyncWriteExt};

/// Boxed future for loading a checkpoint.
pub type CheckpointLoadFuture = BoxFuture<'static, io::Result<Option<BlockLevel>>>;

/// A trait for storing and loading inbox checkpoint.
#[async_trait::async_trait]
pub trait CheckpointStore: Clone + Send + 'static {
    /// Load the checkpoint block level.
    async fn load(&self) -> Result<Option<BlockLevel>>;
    /// Save the checkpoint block level.
    async fn save(&mut self, level: BlockLevel) -> Result<()>;
    /// Returns a boxed future for loading the checkpoint.
    fn load_fut(&self) -> CheckpointLoadFuture {
        let s = self.clone();
        Box::pin(async move { s.load().await })
    }
}

/// JSON structure written to disk
#[derive(Serialize, Deserialize)]
pub(super) struct CheckpointFile {
    pub(super) block_level: BlockLevel,
}

/// Persists the last processed block level to a JSON file.
#[derive(Clone)]
pub struct FileCheckpointStore {
    path: Arc<PathBuf>,
}

impl FileCheckpointStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Arc::new(path.into()),
        }
    }
}

#[async_trait::async_trait]
impl CheckpointStore for FileCheckpointStore {
    /// Load the checkpoint from disk.
    ///
    /// If the file does not exist or is empty, returns `None`.
    async fn load(&self) -> Result<Option<BlockLevel>> {
        match fs::read(&*self.path).await {
            Ok(bytes) => {
                if bytes.is_empty() {
                    return Ok(None);
                }

                let chk: CheckpointFile = serde_json::from_slice(&bytes)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Ok(Some(chk.block_level))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Save the checkpoint to disk safely.
    async fn save(&mut self, level: BlockLevel) -> Result<()> {
        let tmp = self.path.with_extension("tmp");
        let json = serde_json::to_vec(&CheckpointFile { block_level: level })
            .map_err(io::Error::other)?;
        // Write to temp file first and then atomically rename it over the old file.
        // This ensures that file is never left half-written
        {
            let mut f = fs::File::create(&tmp).await?;
            f.write_all(&json).await?;
            f.flush().await?;
            let _ = f.sync_all().await;
        }
        if fs::metadata(&*self.path).await.is_ok() {
            let _ = fs::remove_file(&*self.path).await;
        }
        fs::rename(&tmp, &*self.path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn load_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");
        let store = FileCheckpointStore::new(&path);

        let got = store.load().await.unwrap();
        assert_eq!(got, None);
    }

    #[tokio::test]
    async fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");
        let mut store = FileCheckpointStore::new(&path);

        store.save(42).await.unwrap();
        let got = store.load().await.unwrap();
        assert_eq!(got, Some(42));

        // overwrite with a new value
        store.save(100).await.unwrap();
        let got = store.load().await.unwrap();
        assert_eq!(got, Some(100));
    }

    #[tokio::test]
    async fn corrupted_json_yields_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");
        let store = FileCheckpointStore::new(&path);

        // Write invalid JSON directly
        {
            let mut f = fs::File::create(&path).await.unwrap();
            f.write_all(b"{not json").await.unwrap();
            f.flush().await.unwrap();
        }

        let err = store.load().await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
