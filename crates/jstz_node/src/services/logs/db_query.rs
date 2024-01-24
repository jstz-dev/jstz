use actix_web::{error::ErrorInternalServerError, web::block, Error};
use jstz_proto::context::account::Address;
use serde::{Deserialize, Serialize};

use super::db::{SqliteConnection, SqliteConnectionPool};

type QueryResponseResult = rusqlite::Result<Vec<QueryResponse>>;

#[derive(Debug, Serialize, Deserialize)]
pub enum QueryResponse {
    Log {
        level: String,
        content: String,
        function_address: String,
        request_id: String,
    },
}

type Limit = usize;
type Offset = usize;
pub enum QueryParams {
    GetLogsByAddress(Address, Limit, Offset),
    GetLogsByAddressAndRequestId(Address, String),
}

pub async fn query(
    pool: &SqliteConnectionPool,
    param: QueryParams,
) -> anyhow::Result<Vec<QueryResponse>, Error> {
    let pool = pool.clone();

    let conn = block(move || pool.get())
        .await?
        .map_err(ErrorInternalServerError)?;

    block(move || match param {
        QueryParams::GetLogsByAddress(addr, offset, limit) => {
            logs_by_address(conn, addr, offset, limit)
        }
        QueryParams::GetLogsByAddressAndRequestId(addr, request_id) => {
            logs_by_address_and_request_id(conn, addr, request_id)
        }
    })
    .await?
    .map_err(ErrorInternalServerError)
}

fn logs_by_address(
    connn: SqliteConnection,
    function_address: Address,
    limit: usize,
    offset: usize,
) -> QueryResponseResult {
    let stmt: rusqlite::Statement<'_> = connn.prepare(&format!(
        "SELECT * FROM log WHERE function_address='{}' LIMIT {} OFFSET {}",
        function_address, limit, offset
    ))?;

    collect_logs(stmt)
}

fn logs_by_address_and_request_id(
    connn: SqliteConnection,
    function_address: Address,
    request_id: String,
) -> QueryResponseResult {
    let stmt: rusqlite::Statement<'_> = connn.prepare(&format!(
        "SELECT * FROM log WHERE function_address='{}' AND request_id='{}'",
        function_address, request_id
    ))?;

    collect_logs(stmt)
}

fn collect_logs(mut stmt: rusqlite::Statement<'_>) -> QueryResponseResult {
    let logs = stmt
        .query_map([], |row| {
            Ok(QueryResponse::Log {
                level: row.get(1)?,
                content: row.get(2)?,
                function_address: row.get(3)?,
                request_id: row.get(4)?,
            })
        })?
        .filter_map(Result::ok)
        .collect();

    Ok(logs)
}
