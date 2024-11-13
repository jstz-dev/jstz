use std::sync::Arc;

use anyhow;
use axum::{
    extract::{Path, Query, State},
    response::Sse,
    Json,
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
use serde::Deserialize;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

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
    use crate::services::logs::{LogRecord, Pagination};
    use crate::{
        services::error::{ServiceError, ServiceResult},
        AppState,
    };
    use axum::{
        extract::{Path, Query, State},
        Json,
    };
    use jstz_proto::context::account::Address;

    pub async fn persistent_logs(
        State(AppState { db, .. }): State<AppState>,
        Path(address): Path<String>,
        Query(Pagination { limit, offset }): Query<Pagination>,
    ) -> ServiceResult<Json<Vec<LogRecord>>> {
        let address = Address::from_base58(&address)
            .map_err(|e| ServiceError::BadRequest(e.to_string()))?;
        let result = db.logs_by_address(address, offset, limit).await?;

        Ok(Json(result))
    }

    pub async fn persistent_logs_by_request_id(
        State(AppState { db, .. }): State<AppState>,
        Path(address): Path<String>,
        Path(request_id): Path<String>,
    ) -> ServiceResult<Json<Vec<LogRecord>>> {
        let address = Address::from_base58(&address)
            .map_err(|e| ServiceError::BadRequest(e.to_string()))?;

        let result = db
            .logs_by_address_and_request_id(address, request_id)
            .await?;

        Ok(Json(result))
    }
}

#[cfg(feature = "persistent-logging")]
use persistent_logging::*;

use super::error::{ServiceError, ServiceResult};

// Represents each line in the log file.
pub enum Line {
    #[cfg(feature = "persistent-logging")]
    // Indicates the start and end of a smart function call (request).
    #[cfg(feature = "persistent-logging")]
    Request(RequestEvent),
    // Indicates the js log message from the smart function (e.g. log).
    Js(LogRecord),
}

pub struct LogsService;

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

#[derive(Deserialize, Debug, IntoParams)]
#[serde(default)]
pub struct Pagination {
    limit: usize,
    offset: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

/// Stream console logs
///
/// Returns a stream of console logs from the given Smart Function as Server-Sent Events.
#[utoipa::path(
    get,
    path = "/{address}/stream",
    tag = "Logs",
    responses(
        (status = 200, description = "Successfully connected to log stream as Server-Sent Events"),
        (status = 400),
        (status = 404)
    )
)]
async fn stream_log(
    State(AppState { broadcaster, .. }): State<AppState>,
    Path(address): Path<String>,
) -> ServiceResult<Sse<InfallibleSSeStream>> {
    let address = Address::from_base58(&address)
        .map_err(|e| ServiceError::BadRequest(e.to_string()))?;
    Ok(broadcaster.new_client(address).await)
}

/// Fetch console logs by address
///
/// Fetch console logs by address from the log store only if persistent
/// logging is enabled on this Jstz node instance
#[utoipa::path(
        get,
        path = "/{address}/persistent/requests",
        params(Pagination),
        tag = "Logs",
        responses(
            (status = 200, body = Vec<LogRecord>),
            (status = 400),
            (status = 404)
        )
    )]
#[allow(unused_variables)]
pub async fn persistent_logs(
    app_state: State<AppState>,
    path_params: Path<String>,
    query_params: Query<Pagination>,
) -> ServiceResult<Json<Vec<LogRecord>>> {
    #[cfg(feature = "persistent-logging")]
    return persistent_logging::persistent_logs(app_state, path_params, query_params)
        .await;

    #[cfg(not(feature = "persistent-logging"))]
    Err(ServiceError::PersistentLogsDisabled)
}

/// Fetch console logs by address and request id
///
/// Fetch console logs by address and request id from the log store only if persistent
/// logging is enabled on this Jstz node instance
#[utoipa::path(
        get,
        path = "/{address}/persistent/requests/{request_id}",
        tag = "Logs",
        responses(
            (status = 200, body = Vec<LogRecord>),
            (status = 400),
            (status = 404)
        )
    )]
#[allow(unused_variables)]
pub async fn persistent_logs_by_request_id(
    app_state: State<AppState>,
    addr_path_param: Path<String>,
    request_id_path_param: Path<String>,
) -> ServiceResult<Json<Vec<LogRecord>>> {
    #[cfg(feature = "persistent-logging")]
    return persistent_logging::persistent_logs_by_request_id(
        app_state,
        addr_path_param,
        request_id_path_param,
    )
    .await;

    #[cfg(not(feature = "persistent-logging"))]
    Err(ServiceError::PersistentLogsDisabled)
}

impl Service for LogsService {
    fn router_with_openapi() -> OpenApiRouter<AppState> {
        let router = OpenApiRouter::new()
            .routes(routes!(stream_log))
            .routes(routes!(persistent_logs))
            .routes(routes!(persistent_logs_by_request_id));

        OpenApiRouter::new().nest("/logs", router)
    }
}
