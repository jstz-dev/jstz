#![cfg(feature = "persistent-logging")]
use std::fs;

use super::Line;
use anyhow::{anyhow, Result};
use jstz_core::log_record::LogLevel;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_proto::{context::account::Address, js_logger::LogRecord, logger::RequestEvent};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Params, Statement};
use tokio::task::spawn_blocking;

pub type SqliteConnectionPool = Pool<SqliteConnectionManager>;
pub type SqliteConnection = PooledConnection<r2d2_sqlite::SqliteConnectionManager>;
type QueryResponseResult = Result<Vec<LogRecord>>;

const DB_PATH: &str = ".jstz/log.db";

#[derive(Clone)]
pub struct Db {
    pool: SqliteConnectionPool,
}

impl Db {
    // Initialize the sql databse by createing a connection pool.
    // if the database does not exist, it will be created.
    pub async fn init() -> Result<Self> {
        let db_path = dirs::home_dir()
            .expect("failed to get home directory")
            .join(DB_PATH);

        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let manager = SqliteConnectionManager::file(db_path);
        let pool = SqliteConnectionPool::new(manager)?;

        Self::create_table(pool.clone()).await?;

        Ok(Db { pool })
    }

    async fn create_table(pool: Pool<SqliteConnectionManager>) -> Result<()> {
        let connection = Self::get_connection_from_pool(pool).await?;

        connection.execute_batch(include_str!("./create_db.sql"))?;

        Ok(())
    }

    pub async fn connection(&self) -> Result<SqliteConnection> {
        Self::get_connection_from_pool(self.pool.clone()).await
    }

    async fn get_connection_from_pool(
        pool: SqliteConnectionPool,
    ) -> Result<SqliteConnection> {
        spawn_blocking(move || pool.get())
            .await
            .map_err(|e| {
                anyhow!("Failed to get connection from pool: {}", e.to_string())
            })?
            .map_err(|e| anyhow!("Failed to get connection from pool: {}", e.to_string()))
    }

    // On success, returns the number of rows that were changed/inserted.
    pub(super) async fn flush(&self, line: &Line) -> Result<()> {
        let connection = self.connection().await?;
        match line {
            Line::Request(RequestEvent::Start {
                request_id,
                address,
            }) => connection.execute(
                "INSERT INTO request (id, function_address) VALUES (?1, ?2)",
                (request_id, address.to_string()),
            )?,
            Line::Js(LogRecord {
                request_id,
                address,
                level,
                text,
            }) => connection.execute(
                "INSERT INTO log (level, content, function_address, request_id) VALUES (?1, ?2, ?3, ?4)",
                (
                    level.to_string(),
                    text,
                    address.to_string(),
                    request_id
                ),
            )?,
            // TODO: Update the request row with more fields.
            Line::Request(_) => 0,
        };

        Ok(())
    }

    pub async fn logs_by_address(
        &self,
        function_address: Address,
        limit: usize,
        offset: usize,
    ) -> QueryResponseResult {
        let conn = self.connection().await?;

        let stmt = conn
            .prepare("SELECT * FROM log WHERE function_address = ? LIMIT ? OFFSET ?")?;

        Self::collect_logs(stmt, params![function_address.to_string(), limit, offset])
    }

    pub async fn logs_by_address_and_request_id(
        &self,
        function_address: Address,
        request_id: String,
    ) -> QueryResponseResult {
        let conn = self.connection().await?;

        let stmt = conn
            .prepare("SELECT * FROM log WHERE function_address= ? AND request_id= ?")?;

        Self::collect_logs(stmt, [function_address.to_string(), request_id])
    }

    fn collect_logs<P: Params>(
        mut stmt: Statement<'_>,
        params: P,
    ) -> QueryResponseResult {
        let query_result = stmt
            .query_map(params, |row| {
                Ok((
                    row.get::<usize, String>(1)?,
                    row.get(2)?,
                    row.get::<usize, String>(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(Result::ok);

        // Process logs outside of `query_map` so that anyhow error
        // can be returned on failure.
        let mut logs: Vec<LogRecord> = Vec::new();
        for (level, text, address, request_id) in query_result {
            let log_record = LogRecord {
                level: LogLevel::try_from(level.as_str())
                    .map_err(|e| anyhow!(e.to_string()))?,
                text,
                address: PublicKeyHash::from_base58(address.as_str())?,
                request_id,
            };
            logs.push(log_record)
        }

        Ok(logs)
    }
}
