use std::{
    cell::RefCell,
    fmt::Debug,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;

use log::{debug, error, trace};
use tezos_crypto_rs::base58::{FromBase58Check, ToBase58Check};
use tezos_smart_rollup::{
    core_unsafe::MAX_FILE_CHUNK_SIZE,
    host::{HostError, Runtime, RuntimeError, ValueType},
    storage::path::Path,
    types::{Message, RollupDalParameters, RollupMetadata},
};

use super::db::{exec_delete, exec_delete_glob, exec_read, exec_write, Db};

pub struct Host {
    db: Db,
    preimage_dir: PathBuf,
    log_file: Option<RefCell<File>>,
}

impl Host {
    pub fn new(db: Db, preimage_dir: PathBuf) -> Self {
        Host {
            db,
            preimage_dir,
            log_file: None,
        }
    }

    pub fn with_debug_log_file(
        mut self,
        log_path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let prefix = log_path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        self.log_file.replace(RefCell::new(log_file));
        Ok(self)
    }

    fn connection(
        &self,
    ) -> Result<PooledConnection<SqliteConnectionManager>, RuntimeError> {
        self.db
            .connection()
            .map_err(|e| log_error("db connection", e))
    }
}

fn log_error(name: &str, e: impl Debug) -> RuntimeError {
    debug!("error {name}: {e:?}");
    RuntimeError::HostErr(HostError::GenericInvalidAccess)
}

impl Runtime for Host {
    fn write_output(&mut self, _from: &[u8]) -> Result<(), RuntimeError> {
        trace!("write_output()");
        // Unlike rollup runtime, sequencer runtime does not need to write any output.
        Ok(())
    }

    fn write_debug(&self, msg: &str) {
        match &self.log_file {
            Some(c) => match c.try_borrow_mut() {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(msg.as_bytes()) {
                        error!("failed to write debug log: {e}");
                    };
                    #[cfg(test)]
                    f.flush().unwrap();
                }
                Err(e) => error!("failed to borrow debug log file: {e}"),
            },
            None => debug!("{msg}"),
        }
    }

    fn read_input(&mut self) -> Result<Option<Message>, RuntimeError> {
        // Sequencer runtime does not need to read inbox messages
        unimplemented!()
    }

    fn store_has<T: Path>(&self, path: &T) -> Result<Option<ValueType>, RuntimeError> {
        let log_title = format!("store_has({path})");
        trace!("{log_title}");

        let exists = self
            .db
            .key_exists(&path.to_string())
            .map_err(|e| log_error(&log_title, e))?;

        match exists {
            false => Ok(None),
            true => Ok(Some(ValueType::Value)),
        }
    }

    fn store_read<T: Path>(
        &self,
        path: &T,
        from_offset: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        trace!("store_read({path})");

        let v = self.store_read_all(path)?;
        if from_offset >= v.len() {
            return Err(RuntimeError::HostErr(HostError::StoreInvalidAccess));
        }

        let num_bytes = usize::min(
            MAX_FILE_CHUNK_SIZE,
            usize::min(max_bytes, v.len() - from_offset),
        );
        let mut value = Vec::with_capacity(num_bytes);
        value.extend_from_slice(&v[from_offset..(from_offset + num_bytes)]);
        Ok(value)
    }

    fn store_read_slice<T: Path>(
        &self,
        path: &T,
        from_offset: usize,
        buffer: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        trace!("store_read_slice({path})");
        let v = self.store_read(path, from_offset, MAX_FILE_CHUNK_SIZE)?;

        let size = usize::min(buffer.len(), v.len());
        buffer.copy_from_slice(&v[..size]);

        Ok(size)
    }

    fn store_read_all(&self, path: &impl Path) -> Result<Vec<u8>, RuntimeError> {
        let log_title = format!("store_read_all({path})");
        trace!("{log_title}");

        let client = self.connection()?;
        let read_output = exec_read(&client, &path.to_string())
            .map_err(|e| log_error(&log_title, e))?;

        Ok(match read_output {
            Some(v) => v.from_base58check().map_err(|e| log_error(&log_title, e))?,
            None => vec![],
        })
    }

    fn store_write<T: Path>(
        &mut self,
        path: &T,
        src: &[u8],
        at_offset: usize,
    ) -> Result<(), RuntimeError> {
        let log_title = format!("store_write({path})");
        trace!("{log_title}");

        let mut client = self.connection()?;
        let tx = client.transaction().map_err(|e| log_error(&log_title, e))?;
        let read_output =
            exec_read(&tx, &path.to_string()).map_err(|e| log_error(&log_title, e))?;
        let mut value = match read_output {
            Some(v) => v.from_base58check().map_err(|e| log_error(&log_title, e))?,
            None => vec![],
        };

        if at_offset > value.len() {
            return Err(RuntimeError::HostErr(HostError::StoreInvalidAccess));
        } else if at_offset < value.len() && (at_offset + src.len()) <= value.len() {
            let _ = value
                .splice(at_offset..(at_offset + src.len()), src.iter().copied())
                .collect::<Vec<_>>();
        } else {
            value.truncate(at_offset);
            value.extend_from_slice(src);
        };

        exec_write(&tx, &path.to_string(), &value.to_base58check())
            .map_err(|e| log_error(&log_title, e))?;
        tx.commit().map_err(|e| log_error(&log_title, e))?;
        Ok(())
    }

    fn store_write_all<T: Path>(
        &mut self,
        path: &T,
        src: &[u8],
    ) -> Result<(), RuntimeError> {
        let log_title = format!("store_write_all({path})");
        trace!("{log_title}");

        let client = self.connection()?;
        exec_write(&client, &path.to_string(), &src.to_base58check())
            .map_err(|e| log_error(&log_title, e))
    }

    fn store_delete<T: Path>(&mut self, path: &T) -> Result<(), RuntimeError> {
        let log_title = format!("store_delete({path})");
        trace!("{log_title}");

        let mut client = self.connection()?;
        let tx = client.transaction().map_err(|e| log_error(&log_title, e))?;
        match exec_delete(&tx, &path.to_string()).map_err(|e| log_error(&log_title, e))? {
            0 => return Err(RuntimeError::PathNotFound),
            1 => (),
            _ => unreachable!(),
        }
        exec_delete_glob(&tx, &path.to_string()).map_err(|e| log_error(&log_title, e))?;
        tx.commit().map_err(|e| log_error(&log_title, e))?;
        Ok(())
    }

    fn store_delete_value<T: Path>(&mut self, path: &T) -> Result<(), RuntimeError> {
        let log_title = format!("store_delete_value({path})");
        trace!("{log_title}");

        let client = self.connection()?;
        exec_delete_glob(&client, &path.to_string()).map_err(|e| log_error(&log_title, e))
    }

    fn store_count_subkeys<T: Path>(&self, prefix: &T) -> Result<u64, RuntimeError> {
        let log_title = format!("store_count_subkeys({prefix})");
        trace!("{log_title}");

        let count = self
            .db
            .count_subkeys(&prefix.to_string())
            .map_err(|e| log_error(&log_title, e))?;
        match count {
            Some(v) => Ok(v),
            None => Err(RuntimeError::HostErr(HostError::StoreNotANode)),
        }
    }

    fn store_move(
        &mut self,
        from_path: &impl Path,
        to_path: &impl Path,
    ) -> Result<(), RuntimeError> {
        trace!("store_move({from_path}, {to_path})");
        // This is a bit tricky to implement with sqlite mainly because moving a key means
        // moving all its subkeys and in sqlite all subkeys are stored as separate rows,
        // which means moving a key is very costly. Fortunately, jstz does not use this API
        // for now, so it's left unimplemented.
        unimplemented!()
    }

    fn store_copy(
        &mut self,
        from_path: &impl Path,
        to_path: &impl Path,
    ) -> Result<(), RuntimeError> {
        trace!("store_copy({from_path}, {to_path})");
        // This is a bit tricky to implement with sqlite, similar to store_move. Fortunately,
        // jstz does not use this API for now, so it's left unimplemented.
        unimplemented!()
    }

    fn reveal_preimage(
        &self,
        hash: &[u8; 33],
        destination: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        let hash_str = hex::encode(hash);
        let log_title = format!("reveal_preimage({hash_str})");
        trace!("{log_title}");

        let path = self.preimage_dir.join(hash_str);
        let bytes = std::fs::read(path).map_err(|e| log_error(&log_title, e))?;
        let size = usize::min(destination.len(), bytes.len());
        {
            let (left, _) = destination.split_at_mut(size);
            left.copy_from_slice(&bytes[..size]);
        }
        Ok(size)
    }

    fn reveal_dal_page(
        &self,
        _published_level: i32,
        _slot_index: u8,
        _page_index: i16,
        _destination: &mut [u8],
    ) -> Result<usize, RuntimeError> {
        // Only the kernels executed by a Rollup Node will ever use this function.
        unimplemented!()
    }

    fn reveal_dal_parameters(&self) -> RollupDalParameters {
        // Only the kernels executed by a Rollup Node will ever use this function.
        unimplemented!()
    }

    fn store_value_size(&self, path: &impl Path) -> Result<usize, RuntimeError> {
        trace!("store_value_size({path})");
        let v = self.store_read_all(path)?;
        Ok(v.len())
    }

    fn mark_for_reboot(&mut self) -> Result<(), RuntimeError> {
        // This function is never used, and the underlying logic is not implemented.
        unimplemented!()
    }

    fn reveal_metadata(&self) -> RollupMetadata {
        trace!("reveal_metadata()");
        RollupMetadata {
            // Origination level is not used, so we give a default value
            origination_level: 0,
            raw_rollup_address: [0; 20],
        }
    }

    fn last_run_aborted(&self) -> Result<bool, RuntimeError> {
        // This function is never used, and the underlying logic is not implemented.
        unimplemented!()
    }

    fn upgrade_failed(&self) -> Result<bool, RuntimeError> {
        // This function is never used, and the underlying logic is not implemented.
        unimplemented!()
    }

    fn restart_forced(&self) -> Result<bool, RuntimeError> {
        // This function is never used, and the underlying logic is not implemented.
        unimplemented!()
    }

    fn reboot_left(&self) -> Result<u32, RuntimeError> {
        trace!("reboot_left()");
        // We don’t implement the reboot counter logic, so the value in the tree will always be the
        // same.
        Ok(1001)
    }

    fn runtime_version(&self) -> Result<String, RuntimeError> {
        // This function is never used
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        io::{Read, Seek, Write},
    };

    use jstz_core::host::HostRuntime;
    use log::{Metadata, Record};
    use tempfile::{NamedTempFile, TempDir};
    use tezos_smart_rollup::host::ValueType;
    use tezos_smart_rollup::storage::path::RefPath;

    use crate::sequencer::{db::Db, host::Host};

    thread_local! {
        static LOG_RECORDS: RefCell<Vec<String>> = const {RefCell::new(Vec::new())};
    }

    struct TestLogger;
    impl log::Log for TestLogger {
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &Record<'_>) {
            LOG_RECORDS.with_borrow_mut(|logs| {
                logs.push(format!("{}: {}", record.level(), record.args()));
            });
        }

        fn flush(&self) {}
    }

    #[test]
    fn host_write_debug() {
        static LOGGER: TestLogger = TestLogger {};
        let _ = log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(log::LevelFilter::Debug));
        // clear logs first
        let _ = LOG_RECORDS.take();

        let preimage_dir = TempDir::new().unwrap();
        let log_file = NamedTempFile::new().unwrap();
        let log_file_path = log_file.path();
        let mut host = Host::new(
            Db::init(Some("")).unwrap(),
            preimage_dir.path().to_path_buf(),
        );

        // no log file -- message should show up in the logger
        host.write_debug("message");
        assert_eq!(LOG_RECORDS.take(), vec!["DEBUG: message".to_string()]);

        // now with log file
        host = host.with_debug_log_file(log_file_path).unwrap();

        host.write_debug("foo");
        assert_eq!(LOG_RECORDS.take(), Vec::<String>::new());
        let mut buf = String::new();
        let mut f = std::fs::File::open(log_file_path).unwrap();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "foo");

        // borrow the file to fail write_debug
        let _r = host.log_file.as_ref().unwrap().borrow_mut();
        host.write_debug("bar");
        assert_eq!(
            LOG_RECORDS.take(),
            vec!["ERROR: failed to borrow debug log file: already borrowed".to_string()]
        );
        f.rewind().unwrap();
        buf.clear();
        f.read_to_string(&mut buf).unwrap();
        // `bar` should not show up in the log file
        assert_eq!(buf, "foo");
    }

    #[test]
    fn log_error() {
        static LOGGER: TestLogger = TestLogger {};
        let _ = log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(log::LevelFilter::Debug));
        // clear logs first
        let _ = LOG_RECORDS.take();

        assert_eq!(
            super::log_error("foo", Some("error details")).to_string(),
            "GenericInvalidAccess"
        );
        assert_eq!(
            LOG_RECORDS.take(),
            vec!["DEBUG: error foo: Some(\"error details\")".to_string()]
        );
    }

    #[test]
    fn host() {
        // Covering all host methods in this test to reduce overhead.
        // Sqlite does create a temp db when an empty string is given as db path, but that
        // temp db is only for that specific connection, so we need a temp file here as the
        // db file for the connection pool.
        let db_file = NamedTempFile::new().unwrap();
        let preimage_dir = TempDir::new().unwrap();
        let mut host = Host::new(
            Db::init(Some(db_file.path().to_str().unwrap())).unwrap(),
            preimage_dir.path().to_path_buf(),
        );

        let path = RefPath::assert_from(b"/foo");
        let subkey_path = RefPath::assert_from(b"/foo/s");
        let non_existent_path = RefPath::assert_from(b"/non_existent_path");

        // store_has
        assert!(host.store_has(&path).unwrap().is_none());

        // store_write
        let expected: [u8; 3] = [1, 2, 3];
        host.store_write(&path, &expected, 0).unwrap();

        assert!(matches!(
            host.store_has(&path).unwrap().unwrap(),
            ValueType::Value
        ));

        // store_value_size
        assert_eq!(host.store_value_size(&path).unwrap(), 3);

        // store_count_subkeys when prefix exists
        assert_eq!(host.store_count_subkeys(&path).unwrap(), 1);
        host.store_write_all(&subkey_path, &[1, 2, 3]).unwrap();
        assert_eq!(host.store_count_subkeys(&path).unwrap(), 2);
        for key in ["/a", "/a/b", "/a/c", "/a/d", "/a/d/e", "/b", "/c/d/e"] {
            host.store_write_all(&RefPath::assert_from(key.as_bytes()), &[0])
                .unwrap();
        }
        assert_eq!(
            host.store_count_subkeys(&RefPath::assert_from(b"/a"))
                .unwrap(),
            4
        );
        assert_eq!(
            host.store_count_subkeys(&RefPath::assert_from(b"/a/d"))
                .unwrap(),
            2
        );
        assert_eq!(
            host.store_count_subkeys(&RefPath::assert_from(b"/b"))
                .unwrap(),
            1
        );
        assert_eq!(
            host.store_count_subkeys(&RefPath::assert_from(b"/c"))
                .unwrap(),
            1
        );
        assert!(host
            .store_count_subkeys(&RefPath::assert_from(b"/d"))
            .is_err());

        // store_read
        let v = host.store_read(&path, 2, 100000).unwrap();
        assert_eq!(&v, &[3u8]);
        let v = host.store_read(&path, 10, 100000).unwrap_err().to_string();
        assert_eq!(&v, "StoreInvalidAccess");

        // store_read_all
        let v = host.store_read_all(&path).unwrap();
        assert_eq!(&v, &expected);
        let v = host.store_read_all(&non_existent_path).unwrap();
        assert!(v.is_empty());

        // store_read_slice
        let mut buf = [0u8; 2];
        let v = host.store_read_slice(&path, 1, &mut buf).unwrap();
        assert_eq!(v, 2);
        assert_eq!(&buf, &[2, 3]);

        // store_write part 2: overwrite existing value (1, 2, 3)
        host.store_write(&path, &[1, 2], 1).unwrap();
        let v = host.store_read_all(&path).unwrap();
        assert_eq!(&v, &[1, 1, 2]);
        let v = host
            .store_write(&path, &[1, 2], 10)
            .unwrap_err()
            .to_string();
        assert_eq!(&v, "StoreInvalidAccess");

        // store_write_all
        let expected: [u8; 4] = [4, 5, 6, 7];
        host.store_write_all(&path, &expected).unwrap();
        let v = host.store_read_all(&path).unwrap();
        assert_eq!(&v, &expected);

        // store_delete_value
        host.store_delete_value(&path).unwrap();
        assert!(host.store_has(&subkey_path).unwrap().is_none());
        assert_eq!(host.store_count_subkeys(&path).unwrap(), 1);

        // store_delete
        host.store_delete(&path).unwrap();
        assert!(host.store_has(&path).unwrap().is_none());
        assert_eq!(
            host.store_delete(&non_existent_path)
                .unwrap_err()
                .to_string(),
            "RuntimeError::PathNotFound"
        );

        // write_output
        host.write_output(&[0]).unwrap();

        // reboot_left
        assert_eq!(host.reboot_left().unwrap(), 1001);

        // reveal_metadata
        let metadata = host.reveal_metadata();
        assert_eq!(metadata.raw_rollup_address, [0; 20]);
        assert_eq!(metadata.origination_level, 0);

        // reveal_preimage
        let preimage_hash = [3; 33];
        let hash_str = hex::encode(preimage_hash);
        std::fs::File::create(preimage_dir.path().join(hash_str.clone()))
            .unwrap()
            .write_all(b"abcdefghij")
            .unwrap();

        let mut buf = [0u8; 8];
        assert_eq!(host.reveal_preimage(&preimage_hash, &mut buf).unwrap(), 8);
        assert_eq!(&buf, b"abcdefgh");

        let mut buf = [0u8; 15];
        assert_eq!(host.reveal_preimage(&preimage_hash, &mut buf).unwrap(), 10);
        assert_eq!(&buf, b"abcdefghij\0\0\0\0\0");
    }
}
