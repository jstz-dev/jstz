use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path, Query, ServiceConfig},
    HttpResponse, Responder, Scope,
};

use anyhow;
use jstz_proto::context::account::Address;
use jstz_proto::{
    js_logger::{LogRecord, LOG_PREFIX},
    request_logger::{RequestEvent, REQUEST_END_PREFIX, REQUEST_START_PREFIX},
};
use serde::Deserialize;
use std::io::{self, ErrorKind::InvalidInput};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::Service;

pub mod broadcaster;
mod db;
mod db_query;

use self::{
    broadcaster::Broadcaster,
    db::{SqliteConnectionPool, DB},
    db_query::{query, QueryParams},
};

const DEAULT_PAGINATION_LIMIT: usize = 100;
const DEAULT_PAGINATION_OFFSET: usize = 0;

#[get("{address}/stream")]
async fn stream_logs(
    broadcaster: Data<Broadcaster>,
    path: Path<String>,
) -> io::Result<impl Responder> {
    let address = path.into_inner();

    let address =
        Address::from_base58(&address).map_err(|e| io::Error::new(InvalidInput, e))?;

    Ok(broadcaster.new_client(address).await)
}

#[derive(Deserialize, Debug)]
pub struct Pagination {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[get("{address}/persistent/requests")]
async fn persistent_logs(
    pagination: Query<Pagination>,
    pool: Data<SqliteConnectionPool>,
    path: Path<String>,
) -> anyhow::Result<HttpResponse, actix_web::Error> {
    let address = path.into_inner();

    let address =
        Address::from_base58(&address).map_err(|e| io::Error::new(InvalidInput, e))?;

    let Pagination { limit, offset } = pagination.into_inner();
    let result = query(
        &pool,
        QueryParams::GetLogsByAddress(
            address,
            limit.unwrap_or(DEAULT_PAGINATION_LIMIT),
            offset.unwrap_or(DEAULT_PAGINATION_OFFSET),
        ),
    )
    .await?;

    Ok(HttpResponse::Ok().json(result))
}

#[get("{address}/persistent/requests/{request_id}")]
async fn persistent_logs_by_request_id(
    pool: Data<SqliteConnectionPool>,
    path: Path<(String, String)>,
) -> anyhow::Result<HttpResponse, actix_web::Error> {
    let (address, request_id) = path.into_inner();

    let address =
        Address::from_base58(&address).map_err(|e| io::Error::new(InvalidInput, e))?;

    let result = query(
        &pool,
        QueryParams::GetLogsByAddressAndRequestId(address, request_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(result))
}

pub struct LogsService;

impl Service for LogsService {
    fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/logs")
            .service(stream_logs)
            .service(persistent_logs)
            .service(persistent_logs_by_request_id);

        cfg.service(scope);
    }
}

// Represents each line in the log file.
pub enum Line {
    // Indicates the start and end of a smart function call (request).
    Request(RequestEvent),
    // Indicates the js log message from the smart function (e.g. log).
    Js(LogRecord),
}

impl LogsService {
    // Initalise the LogService by spawning a future that reads the file.
    // The content of the file is broadcasted to clients and flushed to storage.
    pub async fn init(
        log_file_path: String,
        cancellation_token: &CancellationToken,
    ) -> anyhow::Result<(
        Arc<Broadcaster>,
        SqliteConnectionPool,
        JoinHandle<io::Result<()>>,
    )> {
        // Create a broadcaster for streaming logs.
        let broadcaster = Broadcaster::create();
        let broadcaster_arc = Arc::clone(&broadcaster);

        // Create a connection pool for the sqlite database.
        let db_path = dirs::home_dir()
            .expect("failed to get home directory")
            .join(".jstz")
            .join("log.db");
        let db = DB::init(db_path).await?;
        let pool = db.pool();

        let stop_signal = cancellation_token.clone();

        // Spawn a future that reads from the log file.
        // The line is broadcast to client / flushed to storage.
        let tail_file_handle: JoinHandle<io::Result<()>> = actix_web::rt::spawn(
            async move {
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
                                            broadcaster_arc
                                                .broadcast(&log.contract_address, &line_str[LOG_PREFIX.len()..])
                                                .await;
                                        }
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
            },
        );

        Ok((broadcaster, pool, tail_file_handle))
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
