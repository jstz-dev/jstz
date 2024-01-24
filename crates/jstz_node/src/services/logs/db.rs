use super::Line;
use actix_web::web::block;
use anyhow::{anyhow, Result};
use jstz_proto::{js_logger::LogRecord, request_logger::RequestEvent};
use load_file::load_str;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;
pub type SqliteConnectionPool = r2d2::Pool<SqliteConnectionManager>;
pub type SqliteConnection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub struct DB {
    pool: SqliteConnectionPool,
}

impl DB {
    // Initialize the sql databse by createing a connection pool.
    // if the database does not exist, it will be created.
    pub async fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = SqliteConnectionPool::new(manager)?;

        Self::create(pool.clone()).await?;

        Ok(DB { pool })
    }

    async fn create(pool: Pool<SqliteConnectionManager>) -> Result<()> {
        let connection: PooledConnection<SqliteConnectionManager> =
            Self::connection(pool).await?;

        connection
            .execute_batch(load_str!("./create_db.sql"))
            .map_err(|e| anyhow!("Failed to execute create_db.sql: {}", e.to_string()))
    }

    async fn connection(pool: SqliteConnectionPool) -> Result<SqliteConnection> {
        block(move || pool.get())
            .await
            .map_err(|e| {
                anyhow!("Failed to get connection from pool: {}", e.to_string())
            })?
            .map_err(|e| anyhow!("Failed to get connection from pool: {}", e.to_string()))
    }

    pub fn pool(&self) -> SqliteConnectionPool {
        self.pool.clone()
    }

    // On success, returns the number of rows that were changed/inserted.
    pub(super) async fn flush(&self, line: &Line) -> Result<usize> {
        let pool = self.pool();

        let connection: PooledConnection<SqliteConnectionManager> =
            Self::connection(pool).await?;

        match line {
            Line::Request(RequestEvent::Start {
                request_id,
                contract_address,
            }) => connection.execute(
                "INSERT INTO request (id, function_address) VALUES (?1, ?2)",
                (request_id, contract_address.to_string()),
            ).map_err(|e| anyhow!("Failed to insert to db: {}", e.to_string())),
            Line::Js(LogRecord {
                request_id,
                contract_address,
                level,
                text,
            }) => connection.execute(
                "INSERT INTO log (level, content, function_address, request_id) VALUES (?1, ?2, ?3, ?4)",
                (
                    level.to_string(),
                    text,
                    contract_address.to_string(),
                    request_id
                ),
            ).map_err(|e| anyhow!("Failed to insert to db: {}", e.to_string())),
            // TODO: Update the request row with more fields.
            _ => Ok(0),
        }
    }
}
