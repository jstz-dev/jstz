use std::{
    io::{Error, ErrorKind::InvalidInput, Result},
    sync::Arc,
};

use actix_web::{
    get,
    web::{self, ServiceConfig},
    Responder, Scope,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::js_logger::{LogRecord, LOG_PREFIX};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub mod broadcaster;

use crate::{services::Service, tailed_file::TailedFile};
use broadcaster::Broadcaster;

#[get("{address}/stream")]
async fn stream_logs(
    broadcaster: web::Data<Broadcaster>,
    path: web::Path<String>,
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
        path: &str,
        cancellation_token: &CancellationToken,
    ) -> (Arc<Broadcaster>, JoinHandle<Result<()>>) {
        let broadcaster = Broadcaster::create();

        let tail_file_handle: JoinHandle<Result<()>> =
            actix_web::rt::spawn(Self::tail_file(
                path.to_string(),
                Arc::clone(&broadcaster),
                cancellation_token.clone(),
            ));

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
