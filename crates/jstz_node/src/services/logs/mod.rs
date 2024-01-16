use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    Responder, Scope,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::js_logger::{LogRecord, LOG_PREFIX};

use std::io::{Error, ErrorKind::InvalidInput, Result};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use self::broadcaster::Broadcaster;

use super::Service;

pub mod broadcaster;

#[get("{address}/stream")]
async fn stream_logs(
    broadcaster: Data<Broadcaster>,
    path: Path<String>,
) -> Result<impl Responder> {
    let address = path.into_inner();

    // validate address
    let address =
        PublicKeyHash::from_base58(&address).map_err(|e| Error::new(InvalidInput, e))?;

    Ok(broadcaster.new_client(address).await)
}

pub struct LogsService;

impl Service for LogsService {
    fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/logs").service(stream_logs);
        cfg.service(scope);
    }
}

impl LogsService {
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
                        if msg.starts_with(LOG_PREFIX) {
                            let log = LogRecord::try_from_string(&msg[LOG_PREFIX.len()..])
                                .expect("Failed to parse log record from string");
                            broadcaster.broadcast(&log.contract_address, &msg[LOG_PREFIX.len()..]).await;
                        }
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
