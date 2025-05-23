#![allow(unused)]
use std::{fs, path::PathBuf};

use anyhow::Result;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;

pub type SqliteConnectionPool = Pool<SqliteConnectionManager>;

#[derive(Clone)]
pub struct Db {
    pool: SqliteConnectionPool,
}

impl Db {
    /// Initialize the sql databse by createing a connection pool.
    /// If the database does not exist, it will be created.
    ///
    /// Sqlite does create a temp db when an empty string is given as db path, but that
    /// temp db is only for that specific connection, which means it does not work well
    /// with a connection pool (only the first connection established will see changes.)
    pub fn init(path: Option<&str>) -> Result<Self> {
        let manager = match path {
            Some(p) => {
                let db_path = PathBuf::try_from(p)?;
                if let Some(parent) = db_path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                SqliteConnectionManager::file(db_path)
            }
            None => SqliteConnectionManager::memory(),
        };

        let pool = SqliteConnectionPool::new(manager)?;
        Self::create_table(pool.clone())?;

        Ok(Db { pool })
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    fn create_table(pool: Pool<SqliteConnectionManager>) -> Result<()> {
        let conn = pool.get()?;
        conn.execute("CREATE TABLE IF NOT EXISTS jstz_kv (jstz_key TEXT NOT NULL PRIMARY KEY, jstz_value, UNIQUE(jstz_key))", [])?;
        Ok(())
    }

    pub fn key_exists(&self, key: &str) -> Result<bool> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT EXISTS(SELECT 1
                FROM   jstz_kv
                WHERE  jstz_key = ?)"#,
        )?;
        let exists: i32 = stmt.query_row(params![key], |row| row.get(0))?;

        match exists {
            0 => Ok(false),
            1 => Ok(true),
            _ => {
                unreachable!()
            }
        }
    }

    /// Counts subkeys given a prefix. The prefix itself is included. If the prefix does not exist,
    /// `None` is returned.
    pub fn count_subkeys(&self, prefix: &str) -> Result<Option<u64>> {
        // Using glob to find everything that matches `/{prefix}/*`.
        // Since `jstz_key` is indexed and we only need the count,
        // performance should be acceptable.
        let client = self.connection()?;
        let mut glob_prefix = prefix.to_string();
        if !glob_prefix.ends_with("/") {
            glob_prefix += "/";
        }
        glob_prefix += "*";

        let mut stmt = client.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN a.val IS NULL THEN NULL 
                    ELSE a.val + b.val 
                END AS result
            FROM 
                (SELECT 1 AS val FROM jstz_kv WHERE jstz_key = ?1) a,
                (SELECT COUNT(*) AS val FROM jstz_kv WHERE jstz_key GLOB ?2) b"#,
        )?;
        Ok(stmt
            .query_row(params![prefix.to_string(), glob_prefix], |row| row.get(0))
            .optional()?)
    }
}

pub fn exec_read(conn: &Connection, path: &str) -> Result<Option<String>> {
    let result = conn
        // There should be at most one record returned given that jstz_key is the primary key,
        // so it's fine to use `query_row`
        .query_row(
            "SELECT jstz_value FROM jstz_kv WHERE jstz_key = ?",
            [path],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    Ok(result)
}

pub fn exec_write(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO jstz_kv (jstz_key, jstz_value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn exec_delete(conn: &Connection, key: &str) -> Result<usize> {
    Ok(conn.execute("DELETE FROM jstz_kv WHERE jstz_key = ?1", params![key])?)
}

pub fn exec_delete_glob(conn: &Connection, path: &str) -> Result<()> {
    let mut prefix = path.to_string();
    if !prefix.ends_with("/") {
        prefix += "/";
    }
    prefix += "*";

    conn.execute(
        "DELETE FROM jstz_kv WHERE jstz_key GLOB ?1",
        params![prefix],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use tempfile::NamedTempFile;

    use crate::sequencer::db::Db;

    fn insert(conn: &Connection, key: &str) {
        conn.execute(
            "INSERT INTO jstz_kv (jstz_key, jstz_value) VALUES (?1, ?2)",
            params![key, "bar"],
        )
        .unwrap();
    }

    #[test]
    fn create_table() {
        let db = Db::init(Some("")).unwrap();
        let conn = db.connection().unwrap();
        // check if `init` creates the table
        let result = conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='jstz_kv'",
            [],
            |row| row.get::<_, String>(0),
        );
        assert_eq!(result, Ok("jstz_kv".to_string()));
    }

    #[test]
    fn key_exists() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();

        assert!(!db.key_exists("foo").unwrap());

        let conn = db.connection().unwrap();
        insert(&conn, "foo");

        assert!(db.key_exists("foo").unwrap());
    }

    #[test]
    fn count_subkeys() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();

        assert_eq!(db.count_subkeys("foo").unwrap(), None);

        let conn = db.connection().unwrap();
        insert(&conn, "foo");

        assert_eq!(db.count_subkeys("foo").unwrap(), Some(1));

        insert(&conn, "foo/bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(2));

        insert(&conn, "foobar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(2));
    }

    #[test]
    fn exec_read() {
        let key = "/foo";
        let expected = "zzz";
        let db = Db::init(Some("")).unwrap();
        let conn = db.connection().unwrap();

        conn.execute(
            "INSERT INTO jstz_kv (jstz_key, jstz_value) VALUES (?1, ?2)",
            params![key, expected],
        )
        .unwrap();

        let value = super::exec_read(&conn, key).unwrap();
        assert_eq!(value.unwrap(), expected);

        let value = super::exec_read(&conn, "bar").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn exec_write() {
        let key = "/foo";
        let expected = "zzz";
        let db = Db::init(Some("")).unwrap();
        let conn = db.connection().unwrap();

        super::exec_write(&conn, key, expected).unwrap();

        let value = super::exec_read(&conn, key).unwrap();
        assert_eq!(&value.unwrap(), expected);
    }

    #[test]
    fn exec_delete_and_delete_glob() {
        fn count(conn: &Connection) -> u32 {
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM jstz_kv WHERE jstz_key GLOB '/foo/*'")
                .unwrap();
            stmt.query_row(params![], |row| row.get(0)).unwrap()
        }

        let path = "/foo";
        let db = Db::init(Some("")).unwrap();
        let conn = db.connection().unwrap();

        super::exec_write(&conn, &path, "zzz").unwrap();
        assert_eq!(count(&conn), 0);
        // should work when there is no subkey
        assert!(super::exec_delete_glob(&conn, &path).is_ok());

        // create a subkey
        super::exec_write(&conn, "/foo/bar", "aaa").unwrap();
        assert_eq!(count(&conn), 1);
        assert!(super::exec_delete_glob(&conn, &path).is_ok());
        assert_eq!(count(&conn), 0);
        // `/foo` should not be deleted
        assert_eq!(super::exec_read(&conn, &path).unwrap().unwrap(), "zzz");

        // delete `/foo`
        assert_eq!(super::exec_delete(&conn, &path).unwrap(), 1);
        assert!(super::exec_read(&conn, &path).unwrap().is_none());
        assert_eq!(super::exec_delete(&conn, &path).unwrap(), 0);
    }
}
