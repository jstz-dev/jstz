use crate::{
    config::RuntimeEnv,
    sequencer::{
        queue::WrappedOperation,
        riscv_pvm::JstzRiscvPvm,
        runtime::{init_host, process_message},
    },
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
use jstz_proto::operation::internal::InboxId;
use jstz_utils::KeyPair;
use log::{error, info, warn};
use tezos_crypto_rs::hash::SmartRollupHash;
use tezos_smart_rollup::types::SmartRollupAddress;

use super::{db::Db, queue::OperationQueue};
use jstz_kernel::inbox::{encode_signed_operation, ParsedInboxMessage};

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
    runtime_env: &RuntimeEnv,
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> anyhow::Result<Worker> {
    match runtime_env {
        RuntimeEnv::Riscv {
            kernel_path,
            rollup_address,
        } => spawn_riscv_worker(
            queue,
            preimage_dir,
            debug_log_path,
            kernel_path,
            rollup_address,
        ),
        RuntimeEnv::Native => spawn_native_worker(
            queue,
            db,
            injector,
            preimage_dir,
            debug_log_path,
            #[cfg(test)]
            on_exit,
        ),
    }
}

fn spawn_native_worker(
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

fn spawn_riscv_worker(
    queue: Arc<RwLock<OperationQueue>>,
    preimages_dir: PathBuf,
    debug_log_path: Option<&Path>,
    kernel_path: &Path,
    rollup_address: &SmartRollupHash,
) -> anyhow::Result<Worker> {
    let (thread_kill_sig, rx) = channel();
    let heartbeat = Arc::new(AtomicU64::default());
    let debug_log_path = debug_log_path.map(|v| v.to_path_buf());
    let mut pvm = JstzRiscvPvm::new(
        kernel_path,
        rollup_address,
        0,
        Some(preimages_dir.into_boxed_path()),
        heartbeat.clone(),
        debug_log_path,
    )
    .context("failed to launch RISCV PVM")?;

    let rollup_addr = SmartRollupAddress::new(rollup_address.clone());
    Ok(Worker {
        thread_kill_sig,
        heartbeat: heartbeat.clone(),
        inner: Some(spawn_thread(move || {
            info!("RISCV PVM launched");

            'worker: loop {
                let operation = {
                    match queue.write() {
                        Ok(mut q) => q.pop(),
                        Err(e) => {
                            warn!("worker failed to read from queue: {e:?}");
                            None
                        }
                    }
                };
                match operation {
                    Some(op) => {
                        let (inbox_id, encoded_message) = match op {
                            WrappedOperation::FromInbox {
                                original_inbox_message,
                                message,..
                            } => {
                                match hex::decode(original_inbox_message) {
                                    Err(e) => {
                                    // Inbox messages cannot be skipped since eventually rollup will
                                    // execute the messages and sequencer will deviate from rollup.
                                    // Terminating the worker will at least surface the error through
                                    // outdated heartbeats.
                                    error!("worker failed to decode original inbox message: {e}");
                                    break 'worker;
                                },
                                Ok(m) => (message.inbox_id, Ok(m))
                            }
                            },
                            WrappedOperation::FromNode(signed_op) => {
                                (
                                    InboxId{l1_level: 0, l1_message_id: 0},
                                    encode_signed_operation(&signed_op, &rollup_addr).map_err(|e| anyhow::anyhow!("worker failed to encode signed operation into inbox message: {e:?}"))
                                )
                            }
                        };
                        match encoded_message {
                            Ok(message) => {
                                pvm.execute_operation(
                                    inbox_id,
                                    message,
                                    std::ops::Bound::Unbounded,
                                );
                                pvm.dump();
                            }
                            Err(e) => {
                                warn!("{e:?}");
                            }
                        };
                    }
                    _ => std::thread::sleep(Duration::from_millis(100)),
                };

                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }
        })),
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
            &crate::config::RuntimeEnv::Native,
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
            &crate::config::RuntimeEnv::Native,
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
