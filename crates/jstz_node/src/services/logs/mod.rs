use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path},
    Responder,
};

use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::{
    js_logger::{LogRecord, LOG_PREFIX},
    request_logger::{RequestEvent, REQUEST_END_PREFIX, REQUEST_START_PREFIX},
};

use std::io::{Error, ErrorKind::InvalidInput, Result};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub mod broadcaster;
mod db;

use self::{broadcaster::Broadcaster, db::DB};

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

// Represents each line in the log file.
pub(self) enum Line {
    // Indicates the start and end of a smart function call (request).
    Request(RequestEvent),
    // Indicates the js log message from the smart function (e.g. log).
    Js(LogRecord),
}

pub struct LogsService;

impl LogsService {
    // Initalise the LogService by spawning a future that reads the file.
    // The content of the file is broadcasted to clients and flushed to storage.
    pub fn init(
        log_file_path: String,
        db_path: String,
        cancellation_token: &CancellationToken,
    ) -> (Arc<Broadcaster>, JoinHandle<Result<()>>) {
        // Create a broadcaster for streaming logs.
        let broadcaster = Broadcaster::create();

        let db = DB::connect(db_path);

        let stop_signal = cancellation_token.clone();
        let broadcaster_cloned = Arc::clone(&broadcaster);

        // Spawn a future that reads from the log file.
        // The line is broadcast to client / flushed to storage.
        let tail_file_handle: JoinHandle<Result<()>> = actix_web::rt::spawn(async move {
            let file = TailedFile::init(&log_file_path).await?;
            let mut lines = file.lines();
            loop {
                tokio::select! {
                    current_line = lines.next_line() => {
                        if let Ok(Some(line_str)) = current_line {
                            match Self::parse_line(&line_str) {
                                Some(line) => {
                                    let _ = db.flush(&line).await.map_err(|e|
                                        {
                                            println!("Failed to flush log to database: {:?}", e.to_string());
                                        }
                                    );

                                    // Steram the log
                                    if let Line::Js(log) = line {
                                        broadcaster_cloned
                                            .broadcast(&log.contract_address, &line_str[LOG_PREFIX.len()..])
                                            .await;
                                    }
                                }
                                None => ()
                            }
                        }
                    },
                    _ = stop_signal.cancelled() => {
                        break;
                    }
                }
            }

            Ok(())
        });

        (broadcaster, tail_file_handle)
    }

    fn parse_line(line: &str) -> Option<Line> {
        if ![LOG_PREFIX, REQUEST_START_PREFIX, REQUEST_END_PREFIX]
            .iter()
            .any(|pre| line.starts_with(pre))
        {
            return None;
        }

        if line.starts_with(LOG_PREFIX) {
            return LogRecord::try_from_string(&line[LOG_PREFIX.len()..]).map(Line::Js);
        }

        let request_prefix = if line.starts_with(REQUEST_START_PREFIX) {
            REQUEST_START_PREFIX
        } else {
            REQUEST_END_PREFIX
        };

        RequestEvent::try_from_string(&line[request_prefix.len()..]).map(Line::Request)
    }
}
