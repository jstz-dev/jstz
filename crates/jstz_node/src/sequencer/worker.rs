use std::{
    sync::{
        mpsc::{channel, Sender, TryRecvError},
        Arc, RwLock,
    },
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::Duration,
};

use log::warn;

use super::queue::OperationQueue;

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
    #[cfg(test)] on_exit: impl FnOnce() + Send + 'static,
) -> Worker {
    let (thread_kill_sig, rx) = channel();
    Worker {
        thread_kill_sig,
        inner: Some(spawn_thread(move || loop {
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
                Some(_m) => {}
                None => thread::sleep(Duration::from_millis(100)),
            }

            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    #[cfg(test)]
                    on_exit();
                    break;
                }
                Err(TryRecvError::Empty) => {}
            }
        })),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex, RwLock},
        thread,
        time::Duration,
    };

    use crate::sequencer::{queue::OperationQueue, tests::dummy_op};

    #[test]
    fn worker_drop() {
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let v = Arc::new(Mutex::new(0));
        let cp = v.clone();
        let worker = super::spawn(q, move || {
            *cp.lock().unwrap() += 1;
        });

        drop(worker);

        // to ensure that the worker has enough time to pick up the signal
        thread::sleep(Duration::from_millis(800));
        assert_eq!(*v.lock().unwrap(), 1);
    }

    #[test]
    fn worker_consume_queue() {
        let mut q = OperationQueue::new(10);
        let op = dummy_op();
        for _ in 0..10 {
            q.insert(op.clone()).unwrap();
        }
        assert_eq!(q.len(), 10);

        let wrapper = Arc::new(RwLock::new(q));
        let _worker = super::spawn(wrapper.clone(), move || {});

        // to ensure that the worker has enough time to consume the queue
        thread::sleep(Duration::from_millis(800));

        assert_eq!(wrapper.read().unwrap().len(), 0);
    }
}
