use std::{
    sync::mpsc::{channel, Sender, TryRecvError},
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::Duration,
};

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

pub fn spawn(#[cfg(test)] on_exit: impl FnOnce() + Send + 'static) -> Worker {
    let (thread_kill_sig, rx) = channel();
    Worker {
        thread_kill_sig,
        inner: Some(spawn_thread(move || loop {
            thread::sleep(Duration::from_millis(500));

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
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    #[test]
    fn worker_drop() {
        let v = Arc::new(Mutex::new(0));
        let cp = v.clone();
        let worker = super::spawn(move || {
            *cp.lock().unwrap() += 1;
        });

        drop(worker);

        // to ensure that the worker has enough time to pick up the signal
        thread::sleep(Duration::from_millis(800));
        assert_eq!(*v.lock().unwrap(), 1);
    }
}
