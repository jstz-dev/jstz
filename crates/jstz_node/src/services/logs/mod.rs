use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path},
    Responder,
};
use jstz_crypto::public_key_hash::PublicKeyHash;

use std::sync::Arc;
use std::{
    io::{Error, ErrorKind::InvalidInput, Result},
    str::FromStr,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use jstz_api::{LogRecord, LOG_PREFIX};

use self::broadcaster::Broadcaster;

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
                            let log = LogRecord::from_str(&msg[LOG_PREFIX.len()..])
                                .expect("Could not parse log record");
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
