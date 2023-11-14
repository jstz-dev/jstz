use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path},
    Responder,
};

use std::io::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use self::broadcaster::Broadcaster;

pub mod broadcaster;

#[get("{address}/stream")]
async fn stream_logs(
    broadcaster: Data<Broadcaster>,
    path: Path<String>,
) -> Result<impl Responder> {
    let _address = path.into_inner();

    Ok(broadcaster.new_client().await)
}

pub struct LogService;

impl LogService {
    // Initalise the LogService by spawning a future that reads and broadcasts the file
    pub fn init(
        path: String,
        cancellation_token: &CancellationToken,
    ) -> (Arc<Broadcaster>, JoinHandle<Result<()>>) {
        let broadcaster = Broadcaster::create();

        let tail_file_handle: JoinHandle<Result<()>> = actix_web::rt::spawn(
            Self::tail_file(path, Arc::clone(&broadcaster), cancellation_token.clone()),
        );

        (broadcaster, tail_file_handle)
    }

    async fn tail_file(
        path: String,
        broadcaster: Arc<Broadcaster>,
        stop_signal: CancellationToken,
    ) -> std::io::Result<()> {
        let file = TailedFile::init(&path).await?;
        let mut lines = file.lines();
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    if let Ok(Some(msg)) = line {
                        broadcaster.broadcast(&msg).await;
                    }
                },
                _ = stop_signal.cancelled() => {
                    // The stop signal has been triggered.
                    break;
                }
            }
        }
        Ok(())
    }
}
