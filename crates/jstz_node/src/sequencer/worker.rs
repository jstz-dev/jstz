use crate::{
    config::KeyPair,
    sequencer::{
        inbox::parsing::{Message, SequencedOperation},
        runtime::{init_host, process_message},
    },
    services::operations::inject_rollup_message,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::AtomicU64,
        mpsc::{channel, Sender, TryRecvError},
        Arc, RwLock,
    },
    thread::{spawn as spawn_thread, JoinHandle},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use axum::response::IntoResponse;
use jstz_core::BinEncodable;
use log::warn;
use octez::OctezRollupClient;

use super::{db::Db, queue::OperationQueue};

pub struct Worker {
    thread_kill_sig: Sender<()>,
    inner: Option<JoinHandle<()>>,
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
    debug_log_path: Option<&Path>,
    injector: KeyPair,
    rollup_endpoint: String,
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> anyhow::Result<Worker> {
    let (thread_kill_sig, rx) = channel();
    let mut host_rt = init_host(db, preimage_dir).context("failed to init host")?;
    if let Some(p) = debug_log_path {
        host_rt = host_rt
            .with_debug_log_file(p)
            .context("failed to set host debug log file")?;
    }
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .context("failed to build tokio runtime")?;
    let heartbeat = Arc::new(AtomicU64::default());
    Ok(Worker {
        thread_kill_sig,
        heartbeat: heartbeat.clone(),
        inner: Some(spawn_thread(move || {
            let rollup_client = OctezRollupClient::new(rollup_endpoint);
            tokio_rt.block_on(async {
                loop {
                    write_heartbeat(&heartbeat);

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
                            if let Err(e) =
                                process_message(&mut host_rt, op.clone()).await
                            {
                                warn!("error processing message: {e:?}");
                            } else {
                                match sign_message(&injector, op) {
                                    Ok(op) => match op.encode() {
                                        Ok(encoded) => {
                                            if let Err(e) = inject_rollup_message(
                                                encoded,
                                                &rollup_client,
                                            )
                                            .await
                                            {
                                                warn!(
                                                    "failed to inject message: {:?}",
                                                    e.into_response()
                                                );
                                            } else {
                                                warn!(
                                                    "message injected: {:?}",
                                                    op.hash()
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            warn!("failed to encode message: {e:?}")
                                        }
                                    },
                                    Err(e) => warn!("failed to sign message: {e:?}"),
                                }
                            }
                        }
                        None => tokio::time::sleep(Duration::from_millis(100)).await,
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
            })
        })),
    })
}

fn sign_message(signer: &KeyPair, op: Message) -> anyhow::Result<SequencedOperation> {
    let KeyPair(_, secret_key) = signer;
    let signature = secret_key
        .sign(op.hash())
        .map_err(|e| anyhow::anyhow!("failed to sign sequencer operation: {e}"))?;
    Ok(SequencedOperation::new(op.clone(), signature))
}

fn write_heartbeat(heartbeat: &Arc<AtomicU64>) {
    let current_sec = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // safety: this worker should be the only writer
    heartbeat.store(current_sec, std::sync::atomic::Ordering::Relaxed);
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

    use crate::sequencer::{db::Db, queue::OperationQueue, tests::dummy_op};
    use crate::{config::KeyPair, sequencer::inbox::test_utils::hash_of};
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
            KeyPair::default(),
            "http://localhost:8732".to_string(),
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
            Some(log_file.path()),
            KeyPair::default(),
            "http://localhost:8732".to_string(),
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
