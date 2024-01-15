use crate::tailed_file::TailedFile;
use actix_web::{
    error::ErrorInternalServerError,
    get,
    web::{block, Data, Path},
    Error as ActixError, HttpResponse, Responder,
};
use serde::{Deserialize, Serialize};

use core::result;
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

use self::{
    broadcaster::Broadcaster,
    db::{SqliteConnection, SqliteConnectionPool, DB},
};

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

#[derive(Debug, Serialize, Deserialize)]
pub enum LogResult {
    Log { level: String, text: String },
}

#[get("{address}/persistent")]
async fn persistent_logs(
    db: Data<SqliteConnectionPool>,
    path: Path<String>,
) -> result::Result<HttpResponse, ActixError> {
    let address = path.into_inner();

    // validate address
    let _address =
        PublicKeyHash::from_base58(&address).map_err(|e| Error::new(InvalidInput, e))?;

    let result = vec![execute(&db).await?];

    Ok(HttpResponse::Ok().json(result))
}

pub async fn execute(
    pool: &SqliteConnectionPool,
) -> result::Result<Vec<LogResult>, ActixError> {
    let pool = pool.clone();

    let conn = block(move || pool.get())
        .await?
        .map_err(ErrorInternalServerError)?;

    block(move || get_logs(conn))
        .await?
        .map_err(ErrorInternalServerError)
}

fn get_logs(connn: SqliteConnection) -> result::Result<Vec<LogResult>, rusqlite::Error> {
    let mut stmt = connn.prepare("SELECT * FROM log")?;
    let logs = stmt
        .query_map([], |row| {
            Ok(LogResult::Log {
                level: row.get(2)?,
                text: row.get(3)?,
            })
        })
        .and_then(Iterator::collect);

    logs
}

// Represents each line in the log file.
pub enum Line {
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
    ) -> (
        Arc<Broadcaster>,
        SqliteConnectionPool,
        JoinHandle<Result<()>>,
    ) {
        // Create a broadcaster for streaming logs.
        let broadcaster = Broadcaster::create();
        let broadcaster_cloned = Arc::clone(&broadcaster);

        let db = DB::connect(db_path);
        let pool = db.pool();

        let stop_signal = cancellation_token.clone();

        // Spawn a future that reads from the log file.
        // The line is broadcast to client / flushed to storage.
        let tail_file_handle: JoinHandle<Result<()>> = actix_web::rt::spawn(async move {
            let file = TailedFile::init(&log_file_path).await?;
            let mut lines = file.lines();
            loop {
                tokio::select! {
                    current_line = lines.next_line() => {
                        if let Ok(Some(line_str)) = current_line {
                            if let Some(line) = Self::parse_line(&line_str) {
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
                        }
                    },
                    _ = stop_signal.cancelled() => {
                        break;
                    }
                }
            }

            Ok(())
        });

        (broadcaster, pool, tail_file_handle)
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
