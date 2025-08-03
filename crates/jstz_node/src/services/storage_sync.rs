use std::path::PathBuf;

use futures::StreamExt;
use jstz_core::kv::{BatchStorageUpdate, StorageUpdate};
use jstz_utils::event_stream::EventStream;
use tokio::task::AbortHandle;

use crate::sequencer::db::Db;

pub struct StorageSync {
    #[allow(dead_code)]
    abort_handle: AbortHandle,
}

impl StorageSync {
    pub async fn spawn(db: Db, log_path: PathBuf) -> anyhow::Result<Self> {
        let mut stream = EventStream::<BatchStorageUpdate>::from_file(log_path).await?;
        let abort_handle = {
            let task = tokio::spawn({
                println!("Spawning storage sync task");
                let db = db.clone();
                async move {
                    while let Some(mb_updates) = stream.next().await {
                        match mb_updates {
                            Ok(updates) => {
                                for update in updates {
                                    match update {
                                        StorageUpdate::Insert { key, value } => {
                                            db.write(&key, &hex::encode(value)).unwrap();
                                        }
                                        StorageUpdate::Remove { key } => {
                                            db.delete(&key).unwrap();
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error applying updates: {e}");
                            }
                        }
                    }
                }
            });
            task.abort_handle()
        };

        Ok(Self { abort_handle })
    }
}
