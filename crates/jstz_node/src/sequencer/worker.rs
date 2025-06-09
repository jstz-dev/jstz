use crate::sequencer::runtime::{init_host, process_message};
use std::{
    path::PathBuf,
    sync::{
        mpsc::{channel, Sender, TryRecvError},
        Arc,
    },
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::Duration,
};

use anyhow::Context;
use log::warn;
use tokio::sync::RwLock;

use super::{db::Db, queue::OperationQueue};

pub struct Worker {
    thread_kill_sig: Sender<()>,
    inner: Option<JoinHandle<()>>,
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
    let mut host_rt = init_host(db, preimage_dir).context("failed to init host")?;
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .context("failed to build tokio runtime")?;
    Ok(Worker {
        thread_kill_sig,
        // TODO: can use tokio::spawn_blocking to run fully on tokio runtime
        inner: Some(spawn_thread(move || {
            tokio_rt.block_on(async {
                loop {
                    match queue.write().await.pop() {
                        Some(op) => {
                            if let Err(e) = process_message(&mut host_rt, op).await {
                                warn!("error processing message: {e:?}");
                            }
                        }
                        None => thread::sleep(Duration::from_millis(100)),
                    };

                    match rx.try_recv() {
                        Ok(_) | Err(TryRecvError::Disconnected) => {
                            #[cfg(test)]
                            on_exit();
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                    }
                }
            });
        })),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    use tempfile::NamedTempFile;
    use tokio::sync::RwLock;

    use crate::sequencer::{db::Db, queue::OperationQueue, tests::dummy_op};

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

    #[tokio::test]
    async fn worker_consume_queue() {
        let db_file = NamedTempFile::new().unwrap();
        let db = Db::init(Some(db_file.path().to_str().unwrap())).unwrap();
        let mut q = OperationQueue::new(1);
        let op = dummy_op();
        let receipt_key = format!("/jstz_receipt/{}", op.hash());
        q.insert(op.clone()).unwrap();
        assert_eq!(q.len(), 1);
        assert!(!db.key_exists(&receipt_key).unwrap());

        let wrapper = Arc::new(RwLock::new(q));
        let cp = db.clone();
        let _worker = super::spawn(wrapper.clone(), cp, PathBuf::new(), move || {});

        // to ensure that the worker has enough time to consume the queue
        thread::sleep(Duration::from_millis(1000));

        assert_eq!(wrapper.read().await.len(), 0);
        // worker should process the message and the embedded runtime should produce a receipt
        assert!(db.key_exists(&receipt_key).unwrap());
    }
}
