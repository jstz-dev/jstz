use std::path::Path;

use super::Line;
use actix_web::web::block;
use jstz_proto::{js_logger::LogRecord, request_logger::RequestEvent};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;

pub type SqliteConnectionPool = r2d2::Pool<SqliteConnectionManager>;
pub type SqliteConnection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;
pub struct DB {
    pool: SqliteConnectionPool,
}

impl DB {
    // Connects to the database in the given path.
    pub fn connect<P: AsRef<Path>>(path: P) -> Self {
        let manager = SqliteConnectionManager::file(path);
        let pool = SqliteConnectionPool::new(manager).expect(
            "Failed to connect to the database, make sure the db is initalized with `setup_log.db`",
        );

        DB { pool }
    }

    pub fn pool(&self) -> SqliteConnectionPool {
        self.pool.clone()
    }

    pub(super) async fn flush(&self, line: &Line) -> Result<usize> {
        let pool = self.pool();

        let connection: PooledConnection<SqliteConnectionManager> =
            block(move || pool.get())
                .await
                .expect("error running blocking code ")
                .expect("Failed to get connection from pool");

        match line {
            Line::Request(RequestEvent::Start {
                request_id,
                contract_address,
            }) => connection.execute(
                "INSERT INTO request (id, function_address) VALUES (?1, ?2)",
                (request_id, contract_address.to_string()),
            ),
            Line::Js(LogRecord {
                request_id,
                level,
                text,
                ..
            }) => connection.execute(
                "INSERT INTO log (request_id, level, content) VALUES (?1, ?2, ?3)",
                (
                    request_id,
                    serde_json::to_string(&level).expect("failed to serialize log level"),
                    text,
                ),
            ),
            // TODO: Add end of request.
            _ => Ok(0),
        }
    }
}
