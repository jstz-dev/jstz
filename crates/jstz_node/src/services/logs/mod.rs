use crate::tailed_file::TailedFile;
use actix_web::{
    get,
    web::{Data, Path, ServiceConfig},
    Responder, Scope,
};

use super::Service;
use anyhow;
use jstz_proto::context::account::Address;
use jstz_proto::{
    js_logger::{LogRecord, LOG_PREFIX},
    request_logger::{RequestEvent, REQUEST_END_PREFIX, REQUEST_START_PREFIX},
};
use std::io::ErrorKind::InvalidInput;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub mod broadcaster;

#[cfg(feature = "persistent-logging")]
mod db;

#[cfg(not(feature = "persistent-logging"))]
mod db {
    #[derive(Clone)]
    pub struct Db {}
    impl Db {
        pub async fn init() -> anyhow::Result<Self> {
            Ok(Db {})
        }
    }
}

use self::{broadcaster::Broadcaster, db::Db};

#[cfg(feature = "persistent-logging")]
mod persistent_logging {
    pub use crate::{Error, Result};
    pub use actix_web::{web::Query, HttpResponse};
    pub use serde::{Deserialize, Serialize};
    pub const DEAULT_PAGINATION_LIMIT: usize = 100;
    pub const DEAULT_PAGINATION_OFFSET: usize = 0;
    use super::{get, Address, Data, Db, Path};
    #[derive(Deserialize, Debug)]
    pub struct Pagination {
        limit: Option<usize>,
        offset: Option<usize>,
    }

    #[get("{address}/persistent/requests")]
    pub async fn persistent_logs(
        pagination: Query<Pagination>,
        db: Data<Db>,
        path: Path<String>,
    ) -> Result<HttpResponse> {
        let address = path.into_inner();

        let address = Address::from_base58(&address)?;

        let Pagination { limit, offset } = pagination.into_inner();
        let result = query(
            &db,
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
    pub async fn persistent_logs_by_request_id(
        db: Data<Db>,
        path: Path<(String, String)>,
    ) -> Result<HttpResponse> {
        let (address, request_id) = path.into_inner();

        let address = Address::from_base58(&address)?;

        let result = query(
            &db,
            QueryParams::GetLogsByAddressAndRequestId(address, request_id),
        )
        .await?;

        Ok(HttpResponse::Ok().json(result))
    }

    #[derive(Serialize, Deserialize)]
    pub enum QueryResponse {
        Log {
            level: String,
            content: String,
            function_address: String,
            request_id: String,
        },
    }

    /// Queries the log database.
    pub type Limit = usize;
    pub type Offset = usize;
    pub enum QueryParams {
        GetLogsByAddress(Address, Limit, Offset),
        GetLogsByAddressAndRequestId(Address, String),
    }

    pub async fn query(db: &Db, param: QueryParams) -> Result<Vec<QueryResponse>> {
        match param {
            QueryParams::GetLogsByAddress(addr, offset, limit) => {
                db.logs_by_address(addr, offset, limit).await
            }
            QueryParams::GetLogsByAddressAndRequestId(addr, request_id) => {
                db.logs_by_address_and_request_id(addr, request_id).await
            }
        }
        .map_err(Error::InternalError)
    }
}
#[cfg(feature = "persistent-logging")]
use persistent_logging::*;

#[get("{address}/stream")]
async fn stream_logs(
    broadcaster: Data<Broadcaster>,
    path: Path<String>,
) -> std::io::Result<impl Responder> {
    let address = path.into_inner();

    // TODO: add better error
    let address = Address::from_base58(&address)
        .map_err(|e| std::io::Error::new(InvalidInput, e))?;

    Ok(broadcaster.new_client(address).await)
}

pub struct LogsService;

impl Service for LogsService {
    fn configure(cfg: &mut ServiceConfig) {
        let scope = Scope::new("/logs").service(stream_logs);

        #[cfg(not(feature = "persistent-logging"))]
        cfg.service(scope);

        #[cfg(feature = "persistent-logging")]
        {
            let scope = scope
                .service(persistent_logs)
                .service(persistent_logs_by_request_id);
            cfg.service(scope);
        }
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
    // Initalise the LogService by spawning a future that reads and broadcasts the file
    pub async fn init(
        path: &std::path::Path,
        cancellation_token: &CancellationToken,
    ) -> anyhow::Result<(Arc<Broadcaster>, Db, JoinHandle<std::io::Result<()>>)> {
        // Create a broadcaster for streaming logs.
        let broadcaster = Broadcaster::create();

        // Create a connection with the sqlite database.
        let db = Db::init().await?;

        let file = TailedFile::init(path).await?;
        // Spawn a future that reads from the log file.
        // The line is broadcast to client / flushed to storage.
        let tail_file_handle = Self::tail_file(
            file,
            broadcaster.clone(),
            db.clone(),
            cancellation_token.clone(),
        )
        .await;

        Ok((broadcaster, db, tail_file_handle))
    }

    /// Spawn a future that tails log file.
    /// The line is broadcast to client / flushed to storage.
    async fn tail_file(
        file: TailedFile,
        broadcaster: Arc<Broadcaster>,
        #[allow(unused_variables)] db: Db,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<std::io::Result<()>> {
        actix_web::rt::spawn(async move {
            let mut lines = file.lines();
            loop {
                tokio::select! {
                    current_line = lines.next_line() => {
                        if let Ok(Some(line_str)) = current_line {
                            if let Some(line) = Self::parse_line(&line_str) {

                                #[cfg(feature = "persistent-logging")]
                                {
                                    let _ = db.flush(&line).await.map_err(|e|
                                        {
                                            log::warn!("Failed to flush log to database: {:?}", e.to_string());
                                        }
                                    );
                                }

                                // Steram the log
                                #[allow(clippy::collapsible_match)]
                                if let Line::Js(log) = line {
                                    broadcaster
                                        .broadcast(&log.address, &line_str[LOG_PREFIX.len()..])
                                        .await;
                                }
                            }
                        }
                    },
                    _ = cancellation_token.cancelled() => {
                        // The stop signal has been triggered.
                        break;
                    }
                }
            }

            Ok(())
        })
    }

    fn parse_line(line: &str) -> Option<Line> {
        if line.starts_with(LOG_PREFIX) {
            return LogRecord::try_from_string(&line[LOG_PREFIX.len()..]).map(Line::Js);
        }

        if line.starts_with(REQUEST_START_PREFIX) {
            return RequestEvent::try_from_string(&line[REQUEST_START_PREFIX.len()..])
                .map(Line::Request);
        }

        if line.starts_with(REQUEST_END_PREFIX) {
            return RequestEvent::try_from_string(&line[REQUEST_END_PREFIX.len()..])
                .map(Line::Request);
        }

        None
    }
}
