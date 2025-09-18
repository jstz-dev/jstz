use crate::sequencer::runtime::{init_host, process_message};
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
use jstz_utils::KeyPair;
use log::warn;

use super::{db::Db, queue::OperationQueue};
use jstz_kernel::inbox::ParsedInboxMessage;

#[cfg(feature = "oracle")]
use jstz_kernel::inbox::LevelInfo;

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
    injector: &KeyPair,
    preimage_dir: PathBuf,
    debug_log_path: Option<&Path>,
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> anyhow::Result<Worker> {
    let (thread_kill_sig, rx) = channel();
    let mut host_rt =
        init_host(db, preimage_dir, injector).context("failed to init host")?;
    if let Some(p) = debug_log_path {
        host_rt = host_rt
            .with_debug_log_file(p)
            .context("failed to set host debug log file")?;
    }
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .context("failed to build tokio runtime")?;
    let heartbeat = Arc::new(AtomicU64::default());
    Ok(Worker {
        thread_kill_sig,
        heartbeat: heartbeat.clone(),
        inner: Some(spawn_thread(move || {
            #[cfg(feature = "oracle")]
            run_event_loop(
                tokio_rt,
                host_rt,
                queue,
                heartbeat,
                rx,
                #[cfg(test)]
                on_exit,
            );

            #[cfg(not(feature = "oracle"))]
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
                            if let ParsedInboxMessage::JstzMessage(message) =
                                op.to_message()
                            {
                                if let Err(e) =
                                    process_message(&mut host_rt, message).await
                                {
                                    warn!("error processing message: {e:?}");
                                }
                            }
                        }
                        _ => tokio::time::sleep(Duration::from_millis(100)).await,
                    }

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

#[cfg(feature = "oracle")]
// See [jstz_kernel::riscv_kernel::run_event_loop]
fn run_event_loop(
    tokio_rt: tokio::runtime::Runtime,
    mut host: super::host::Host,
    queue: Arc<RwLock<OperationQueue>>,
    heartbeat: Arc<AtomicU64>,
    rx: std::sync::mpsc::Receiver<()>,
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) {
    let local_set = tokio::task::LocalSet::new();
    jstz_proto::runtime::ProtocolContext::init_global(&mut host, 0).unwrap(); // unwrap to propagate error
    local_set.block_on(&tokio_rt, async {
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
                Some(wrapper) => match wrapper.to_message() {
                    ParsedInboxMessage::JstzMessage(op) => {
                        let mut hrt = host.clone();
                        local_set.spawn_local(async move {
                            if let Err(e) = process_message(&mut hrt, op).await {
                                warn!("error processing message: {e:?}");
                            }
                        });
                        tokio::task::yield_now().await;
                        tokio::task::yield_now().await;
                    }
                    ParsedInboxMessage::LevelInfo(LevelInfo::Start) => {
                        let mut hrt = host.clone();
                        let ctx = jstz_proto::runtime::PROTOCOL_CONTEXT
                            .get()
                            .expect("Protocol context should be initialized");
                        ctx.increment_level();
                        let oracle_ctx = ctx.oracle();
                        let mut oracle = oracle_ctx.lock();
                        oracle.gc_timeout_requests(&mut hrt);
                        tokio::task::yield_now().await;
                    }
                    _ => (),
                },
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
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
}

pub(crate) fn write_heartbeat(heartbeat: &Arc<AtomicU64>) {
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
    use crate::{sequencer::inbox::test_utils::hash_of, test::default_injector};
    use tempfile::NamedTempFile;

    #[test]
    fn worker_drop() {
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let v = Arc::new(Mutex::new(0));
        let cp = v.clone();
        let worker = super::spawn(
            q,
            Db::init(Some("")).unwrap(),
            &default_injector(),
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
            &default_injector(),
            PathBuf::new(),
            Some(log_file.path()),
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
            buf.contains("Smart function deployed: KT1H4GfcBgx11M8ri6wwyDtbMUbqYfDQ7WmU")
        );
    }
}
