use crate::sequencer::runtime::{init_host, process_message};
use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicU64,
        mpsc::{channel, Sender, TryRecvError},
        Arc, RwLock,
    },
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, bail, Context};
use log::warn;

use super::{db::Db, queue::OperationQueue};

pub struct Worker {
    thread_kill_sig: Sender<()>,
    inner: Option<JoinHandle<Result<(), std::io::Error>>>,
    heartbeat: Arc<AtomicU64>,
}

impl Worker {
    pub fn heartbeat(&self) -> Arc<AtomicU64> {
        self.heartbeat.clone()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.thread_kill_sig.send(());
        if let Some(h) = self.inner.take() {
            let _ = h.join();
        }
    }
}

pub fn spawn(
    queue: Arc<RwLock<OperationQueue>>,
    db: Db,
    preimage_dir: PathBuf,
    debug_log_path: Option<PathBuf>,
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> anyhow::Result<Worker> {
    let (thread_kill_sig, rx) = channel();
    let mut rt = init_host(db, preimage_dir).context("failed to init host")?;
    if let Some(p) = debug_log_path {
        rt = rt
            .with_debug_log_file(&p)
            .context("failed to set host debug log file")?;
    }

    let heartbeat = Arc::new(AtomicU64::default());
    Ok(Worker {
        thread_kill_sig,
        heartbeat: heartbeat.clone(),
        inner: {
            let thread =
                spawn_thread(move || match tokio::runtime::Builder::new_current_thread()
                    .build()
                {
                    Ok(tokio) => loop {
                        let current_sec = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        // safety: this worker should be the only writer
                        heartbeat
                            .store(current_sec, std::sync::atomic::Ordering::Relaxed);
                        let v = {
                            match queue.write() {
                                Ok(mut q) => q.pop(),
                                Err(e) => {
                                    warn!("worker failed to read from queue: {e:?}");
                                    None
                                }
                            }
                        };

                        match v {
                            Some(op) => {
                                if let Err(e) = process_message(&tokio, &mut rt, op) {
                                    warn!("error processing message: {e:?}");
                                }
                            }
                            None => thread::sleep(Duration::from_millis(100)),
                        };

                        match rx.try_recv() {
                            Ok(_) | Err(TryRecvError::Disconnected) => {
                                #[cfg(test)]
                                on_exit();
                                break Ok(());
                            }
                            Err(TryRecvError::Empty) => {}
                        }
                    },
                    Err(e) => Err(e),
                });

            if thread.is_finished() {
                // Thread could not start
                let result = thread
                    .join()
                    .map_err(|_| anyhow!("Thread could not start"))?;

                // Tokio failed
                result?;

                // Thread ended early
                bail!("Thread ended early")
            } else {
                Some(thread)
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use std::{
        io::Read,
        path::PathBuf,
        sync::{Arc, Mutex, RwLock},
        thread,
        time::Duration,
    };

    use crate::sequencer::inbox::test_utils::hash_of;
    use crate::sequencer::{db::Db, queue::OperationQueue, tests::dummy_op};
    use tempfile::NamedTempFile;

    #[test]
    fn worker_drop() {
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let v = Arc::new(Mutex::new(0));
        let cp = v.clone();
        let worker = super::spawn(
            q,
            Db::init(Some("")).unwrap(),
            PathBuf::new(),
            None,
            move || {
                *cp.lock().unwrap() += 1;
            },
        )
        .unwrap();

        let h = worker.heartbeat();
        let t1 = h.load(std::sync::atomic::Ordering::Relaxed);
        thread::sleep(Duration::from_millis(1100));
        // heartbeat should increment by at least 1
        let t2 = h.load(std::sync::atomic::Ordering::Relaxed);
        assert!(t2 > t1);

        drop(worker);

        // to ensure that the worker has enough time to pick up the signal
        thread::sleep(Duration::from_millis(2000));
        assert_eq!(*v.lock().unwrap(), 1);
        // heartbeat should not keep increasing
        let t3 = h.load(std::sync::atomic::Ordering::Relaxed);
        assert!(t3 - t2 <= 1);
    }

    #[test]
    fn worker_consume_queue() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut log_file = NamedTempFile::new().unwrap();
        let mut q = OperationQueue::new(1);
        let op = dummy_op();
        let receipt_key = format!("/jstz_receipt/{}", hash_of(&op));
        q.insert(op.clone()).unwrap();
        assert_eq!(q.len(), 1);
        assert!(!db.key_exists(&receipt_key).unwrap());

        let wrapper = Arc::new(RwLock::new(q));
        let cp = db.clone();
        let _worker = super::spawn(
            wrapper.clone(),
            cp,
            PathBuf::new(),
            Some(log_file.path().to_path_buf()),
            move || {},
        );

        // to ensure that the worker has enough time to consume the queue
        thread::sleep(Duration::from_millis(1000));

        assert_eq!(wrapper.read().unwrap().len(), 0);
        // worker should process the message and the embedded runtime should produce a receipt
        assert!(db.key_exists(&receipt_key).unwrap());
        // check logs
        let mut buf = String::new();
        log_file.read_to_string(&mut buf).unwrap();
        assert!(
            buf.contains("Smart function deployed: KT19xhZJaQkEiVo6w3uRZor6VY5Z9KXZkQ1N")
        );
    }
}
