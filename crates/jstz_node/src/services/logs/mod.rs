use std::sync::Arc;

use anyhow;
use axum::{
    extract::{Path, State},
    response::Sse,
    routing::get,
};
use broadcaster::InfallibleSSeStream;
#[cfg(feature = "persistent-logging")]
use jstz_proto::request_logger::{
    RequestEvent, REQUEST_END_PREFIX, REQUEST_START_PREFIX,
};
use jstz_proto::{
    context::account::Address,
    js_logger::{LogRecord, LOG_PREFIX},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use utoipa_axum::router::OpenApiRouter;

use crate::{tailed_file::TailedFile, AppState, Service};

pub mod broadcaster;

#[cfg(feature = "persistent-logging")]
pub mod db;

#[cfg(not(feature = "persistent-logging"))]
pub mod db {
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
    use crate::{services::error::ServiceError, AppState};
    pub use anyhow::Result;
    use axum::{
        extract::{Path, Query, State},
        response::IntoResponse,
        Json,
    };
    use jstz_proto::context::account::Address;
    pub use serde::{Deserialize, Serialize};

    use super::db::Db;
    pub const DEAULT_PAGINATION_LIMIT: usize = 100;
    pub const DEAULT_PAGINATION_OFFSET: usize = 0;

    #[derive(Deserialize, Debug)]
    pub struct Pagination {
        limit: Option<usize>,
        offset: Option<usize>,
    }

    pub async fn persistent_logs(
        State(AppState { db, .. }): State<AppState>,
        Path(address): Path<String>,
        Query(Pagination { limit, offset }): Query<Pagination>,
    ) -> anyhow::Result<impl IntoResponse, ServiceError> {
        let address = Address::from_base58(&address)
            .map_err(|e| ServiceError::BadRequest(e.to_string()))?;
        let result = query(
            &db,
            QueryParams::GetLogsByAddress(
                address,
                limit.unwrap_or(DEAULT_PAGINATION_LIMIT),
                offset.unwrap_or(DEAULT_PAGINATION_OFFSET),
            ),
        )
        .await?;

        Ok(Json(result))
    }

    pub async fn persistent_logs_by_request_id(
        State(AppState { db, .. }): State<AppState>,
        Path(address): Path<String>,
        Path(request_id): Path<String>,
    ) -> Result<impl IntoResponse, ServiceError> {
        let address = Address::from_base58(&address)
            .map_err(|e| ServiceError::BadRequest(e.to_string()))?;

        let result = query(
            &db,
            QueryParams::GetLogsByAddressAndRequestId(address, request_id),
        )
        .await?;

        Ok(Json(result))
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

    pub async fn query(
        db: &Db,
        param: QueryParams,
    ) -> anyhow::Result<Vec<QueryResponse>> {
        match param {
            QueryParams::GetLogsByAddress(addr, offset, limit) => {
                db.logs_by_address(addr, offset, limit).await
            }
            QueryParams::GetLogsByAddressAndRequestId(addr, request_id) => {
                db.logs_by_address_and_request_id(addr, request_id).await
            }
        }
    }
}
#[cfg(feature = "persistent-logging")]
use persistent_logging::*;

use super::error::{ServiceError, ServiceResult};

pub struct LogsService;

impl Service for LogsService {
    fn router_with_openapi() -> OpenApiRouter<AppState> {
        let routes = OpenApiRouter::new().route("/:address/stream", get(stream_log));
        #[cfg(feature = "persistent-logging")]
        let routes = routes
            .route("/:address/persistent/requests", get(persistent_logs))
            .route(
                "/:address/persistent/requests/:request_id",
                get(persistent_logs_by_request_id),
            );
        OpenApiRouter::new().nest("/logs", routes)
    }
}

// Represents each line in the log file.
pub enum Line {
    #[cfg(feature = "persistent-logging")]
    // Indicates the start and end of a smart function call (request).
    #[cfg(feature = "persistent-logging")]
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
        let broadcaster = Broadcaster::new();

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
        tokio::task::spawn(async move {
            let mut lines = file.lines();
            loop {
                tokio::select! {
                    current_line = lines.next_line() => {
                        if let Ok(Some(line_str)) = current_line {
                            // CLIPPY
                            // The collapsible-match lint gives a false positive for this line since
                            // it doesn't consider the line below (guarded by the 'persistent-logging' feature flag)
                            #[allow(clippy::collapsible_match)]
                            if let Some(line) = Self::parse_line(&line_str) {

                                #[cfg(feature = "persistent-logging")]
                                {
                                    let _ = db.flush(&line).await.map_err(|e|
                                        {
                                            log::warn!("Failed to flush log to database: {:?}", e.to_string());
                                        }
                                    );
                                }

                                // Stream the log
                                #[cfg(not(feature = "persistent-logging"))]
                                #[allow(irrefutable_let_patterns)]
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
        if let Some(log) = line.strip_prefix(LOG_PREFIX) {
            return LogRecord::try_from_string(log).map(Line::Js);
        }

        #[cfg(feature = "persistent-logging")]
        {
            if let Some(request) = line.strip_prefix(REQUEST_START_PREFIX) {
                return RequestEvent::try_from_string(request).map(Line::Request);
            }

            if let Some(request) = line.strip_prefix(REQUEST_END_PREFIX) {
                return RequestEvent::try_from_string(request).map(Line::Request);
            }
        }

        None
    }
}

async fn stream_log(
    State(AppState { broadcaster, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Sse<InfallibleSSeStream>> {
    let address = Address::from_base58(&address)
        .map_err(|e| ServiceError::BadRequest(e.to_string()))?;
    Ok(broadcaster.new_client(address).await)
}
