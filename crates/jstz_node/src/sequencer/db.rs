use std::{fs, path::PathBuf};

use anyhow::Context;
use anyhow::Result;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;

pub type SqliteConnectionPool = Pool<SqliteConnectionManager>;

/// Database wrapper that manipulates the sequencer database.
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
                let db_path = PathBuf::from(p);
                if let Some(parent) = db_path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                SqliteConnectionManager::file(db_path)
            }
            None => SqliteConnectionManager::memory(),
        }
        // Documentation says that a default busy timeout of 5 seconds is set for each connection
        // so this shouldn't matter. Setting it explicitly here just to be safe.
        .with_init(|c| c.busy_timeout(std::time::Duration::from_secs(5)));

        let pool = SqliteConnectionPool::new(manager)?;
        Self::setup(pool.clone())?;

        Ok(Db { pool })
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        let conn = self
            .pool
            .get()
            .context("failed to get connection from pool")?;
        Ok(conn)
    }

    fn setup(pool: Pool<SqliteConnectionManager>) -> Result<()> {
        let conn = pool.get().context("failed to get connection from pool")?;
        conn.execute("CREATE TABLE IF NOT EXISTS jstz_kv (jstz_key TEXT NOT NULL PRIMARY KEY, jstz_value, UNIQUE(jstz_key))", []).context("failed to create table")?;
        // Allows reads while writes are taking place. This works when there is only one writer
        // and is fine in our use case.
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("failed to set pragma journal mode")?;
        // Reduces the frequency that sqlite synchronises with the journal. NORMAL is fine with WAL.
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("failed to set pragma synchronous")?;

        Ok(())
    }

    /// Checks if a key exists.
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
    /// i.e. it itself does not possess any value AND there is no other key with the prefix,
    /// `None` is returned.
    pub fn count_subkeys(&self, prefix: &str) -> Result<Option<u64>> {
        let client = self.connection()?;

        // This is basically `get_subkeys` wrapped by `COUNT` in order to skip unnecessary
        // return values.
        let mut stmt = client.prepare(
            r#"
            SELECT COUNT(*)
            FROM (
                SELECT SUBSTR(jstz_key, LENGTH(?2))
                FROM jstz_kv
                WHERE jstz_key = ?1
                OR jstz_key GLOB ?2
                AND NOT jstz_key GLOB ?3
                UNION
                SELECT DISTINCT SUBSTR(SUBSTR(jstz_key, LENGTH(?2)), 0, INSTR(SUBSTR(jstz_key, LENGTH(?2)), '/')) AS child
                FROM jstz_kv
                WHERE jstz_key GLOB ?3
            )"#,
        )?;
        let res = stmt.query_row(
            params![prefix, format!("{prefix}/*"), format!("{prefix}/*/*"),],
            |row| row.get(0),
        )?;
        Ok(if res == 0 { None } else { Some(res) })
    }

    /// Reads subkeys given a prefix. The prefix itself is included as an empty string. If the
    /// prefix does not exist, i.e. it itself does not possess any value AND there is no other
    /// key with the prefix, `None` is returned.
    pub fn get_subkeys(&self, prefix: &str) -> Result<Option<Vec<String>>> {
        // Using glob to find the subtree, i.e. everything that matches `/{prefix}/*`.
        // Since `jstz_key` is indexed, performance should be acceptable.
        let client = self.connection()?;

        // Union of two parts:
        // 1. Select the prefix itself and its immediate subkeys. `SUBSTR` removes the prefix since
        // we only want the subkeys themselves.
        // 2. Select everything else that matches the prefix and keep the immediate subkeys.
        // The nested SUBSTR part:
        // - SUBSTR(jstz_key, LENGTH(?2)): input string -- selected key with prefix removed
        // - 0: find a substring of the input string starting from character index 0
        // - INSTR(SUBSTR(jstz_key, LENGTH(?2)), '/'): returns the index where the first slash is in
        //   the input string. This is essentially the length of the immediate subkey.
        // The nested SUBSTR therefore means "returning a substring of the input (selected key with
        // prefix removed) from the beginning to the first occurrence of the slash character".
        let mut stmt = client.prepare(
            r#"
            SELECT SUBSTR(jstz_key, LENGTH(?2))
            FROM jstz_kv
            WHERE jstz_key = ?1
                OR jstz_key GLOB ?2
                AND NOT jstz_key GLOB ?3
            UNION
            SELECT DISTINCT SUBSTR(SUBSTR(jstz_key, LENGTH(?2)), 0, INSTR(SUBSTR(jstz_key, LENGTH(?2)), '/')) AS tmp
            FROM jstz_kv
            WHERE jstz_key GLOB ?3"#,
        )?;
        let mut rows = stmt.query(params![
            prefix,
            format!("{prefix}/*"),
            format!("{prefix}/*/*"),
        ])?;
        let mut keys = vec![];
        while let Some(r) = rows.next()? {
            keys.push(r.get(0)?);
        }
        Ok(if keys.is_empty() { None } else { Some(keys) })
    }

    pub fn read_key(&self, key: &str) -> Result<Option<String>> {
        let conn = self.connection()?;
        exec_read(&conn, key)
    }

    pub fn write(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.connection()?;
        exec_write(&conn, key, value)
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        let conn = self.connection()?;
        exec_delete_glob(&conn, key)
    }
}

/// Reads a row using an existing database connection.
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

/// Inserts a record using an existing database connection.
pub fn exec_write(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO jstz_kv (jstz_key, jstz_value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Deletes a row using an existing database connection.
pub fn exec_delete(conn: &Connection, key: &str) -> Result<usize> {
    Ok(conn.execute("DELETE FROM jstz_kv WHERE jstz_key = ?1", params![key])?)
}

/// Deletes rows whose keys match a given prefix using an existing database connection.
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
    use rusqlite::{params, Connection, OptionalExtension};
    use tempfile::NamedTempFile;

    use crate::sequencer::db::Db;

    fn insert(conn: &Connection, key: &str, value: &str) {
        conn.execute(
            "INSERT INTO jstz_kv (jstz_key, jstz_value) VALUES (?1, ?2)",
            params![key, value],
        )
        .unwrap();
    }

    fn read_row(conn: &Connection, key: &str) -> Option<String> {
        conn.query_row(
            "SELECT jstz_value FROM jstz_kv WHERE jstz_key = ?",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap()
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
        insert(&conn, "foo", "bar");

        assert!(db.key_exists("foo").unwrap());
    }

    #[test]
    fn count_subkeys() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();

        // should be none because the key does not exist
        assert_eq!(db.count_subkeys("foo").unwrap(), None);

        let conn = db.connection().unwrap();
        insert(&conn, "foo", "bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(1));

        insert(&conn, "foobar", "bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(1));

        insert(&conn, "foo/bar", "bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(2));
        assert_eq!(db.count_subkeys("foo/bar").unwrap(), Some(1));

        insert(&conn, "foo/bar/baz", "bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(2));
        assert_eq!(db.count_subkeys("foo/bar").unwrap(), Some(2));
        assert_eq!(db.count_subkeys("foo/bar/baz").unwrap(), Some(1));

        insert(&conn, "foo/bar/qux", "bar");
        assert_eq!(db.count_subkeys("foo").unwrap(), Some(2));
        assert_eq!(db.count_subkeys("foo/bar").unwrap(), Some(3));
        assert_eq!(db.count_subkeys("foo/bar/qux").unwrap(), Some(1));
    }

    #[test]
    fn exec_read_and_read_key() {
        let key = "/foo";
        let expected = "zzz";
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let conn = db.connection().unwrap();

        insert(&conn, key, expected);

        let value = super::exec_read(&conn, key).unwrap();
        assert_eq!(value.unwrap(), expected);

        let value = db.read_key(key).unwrap();
        assert_eq!(value.unwrap(), expected);

        let value = super::exec_read(&conn, "bar").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn exec_write_and_write() {
        let key = "/foo";
        let expected = "zzz";
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let conn = db.connection().unwrap();

        super::exec_write(&conn, key, expected).unwrap();

        let value = read_row(&conn, key);
        assert_eq!(&value.unwrap(), expected);

        let expected = "abc";
        db.write(key, expected).unwrap();

        let value = read_row(&conn, key);
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

        insert(&conn, path, "zzz");
        assert_eq!(count(&conn), 0);
        // should work when there is no subkey
        assert!(super::exec_delete_glob(&conn, path).is_ok());

        // create a subkey
        insert(&conn, "/foo/bar", "aaa");
        assert_eq!(count(&conn), 1);
        assert!(super::exec_delete_glob(&conn, path).is_ok());
        assert_eq!(count(&conn), 0);
        // `/foo` should not be deleted
        assert_eq!(read_row(&conn, path).unwrap(), "zzz");

        // delete `/foo`
        assert_eq!(super::exec_delete(&conn, path).unwrap(), 1);
        assert!(read_row(&conn, path).is_none());
        assert_eq!(super::exec_delete(&conn, path).unwrap(), 0);
    }

    #[test]
    fn get_subkeys() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let conn = db.connection().unwrap();

        for key in [
            "foo",
            "foo/aa",
            "foo/bb",
            "foo/cc",
            "foo/cc/dd",
            "bar",
            "baz/aa",
        ] {
            insert(&conn, key, "1");
        }

        let mut keys = db.get_subkeys("foo").unwrap().unwrap();
        keys.sort();
        assert_eq!(keys, ["", "aa", "bb", "cc"]);

        let mut keys = db.get_subkeys("foo/cc").unwrap().unwrap();
        keys.sort();
        assert_eq!(keys, ["", "dd"]);

        let mut keys = db.get_subkeys("bar").unwrap().unwrap();
        keys.sort();
        assert_eq!(keys, [""]);

        let mut keys = db.get_subkeys("baz").unwrap().unwrap();
        keys.sort();
        assert_eq!(keys, ["aa"]);

        assert!(db.get_subkeys("nonsense").unwrap().is_none());
    }
}
