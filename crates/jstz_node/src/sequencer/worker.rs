use crate::sequencer::runtime::{init_host, process_message};
use std::{
    path::PathBuf,
    sync::{
        mpsc::{channel, Sender, TryRecvError},
        Arc, RwLock,
    },
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::Duration,
};

use anyhow::{anyhow, bail, Context};
use log::warn;

use super::{db::Db, queue::OperationQueue};

pub struct Worker {
    thread_kill_sig: Sender<()>,
    inner: Option<JoinHandle<Result<(), std::io::Error>>>,
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
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> anyhow::Result<Worker> {
    let (thread_kill_sig, rx) = channel();
    let mut rt = init_host(db, preimage_dir).context("failed to init host")?;
    Ok(Worker {
        thread_kill_sig,
        inner: {
            let thread =
                spawn_thread(move || match tokio::runtime::Builder::new_current_thread()
                    .build()
                {
                    Ok(tokio) => loop {
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
        let worker =
            super::spawn(q, Db::init(Some("")).unwrap(), PathBuf::new(), move || {
                *cp.lock().unwrap() += 1;
            });

        drop(worker);

        // to ensure that the worker has enough time to pick up the signal
        thread::sleep(Duration::from_millis(800));
        assert_eq!(*v.lock().unwrap(), 1);
    }

    #[test]
    fn worker_consume_queue() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut q = OperationQueue::new(1);
        let op = dummy_op();
        let receipt_key = format!("/jstz_receipt/{}", hash_of(&op));
        q.insert(op.clone()).unwrap();
        assert_eq!(q.len(), 1);
        assert!(!db.key_exists(&receipt_key).unwrap());

        let wrapper = Arc::new(RwLock::new(q));
        let cp = db.clone();
        let _worker = super::spawn(wrapper.clone(), cp, PathBuf::new(), move || {});

        // to ensure that the worker has enough time to consume the queue
        thread::sleep(Duration::from_millis(1000));

        assert_eq!(wrapper.read().unwrap().len(), 0);
        // worker should process the message and the embedded runtime should produce a receipt
        assert!(db.key_exists(&receipt_key).unwrap());
    }
}
