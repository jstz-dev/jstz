use std::{
    sync::{
        mpsc::{channel, Sender, TryRecvError},
        Arc,
    },
    thread::{self, spawn as spawn_thread, JoinHandle},
    time::Duration,
};

#[derive(Clone)]
pub struct Worker {
    tx: Sender<()>,
    _inner: Arc<JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(());
    }
}

pub fn spawn(on_exit: impl FnOnce() -> () + Send + 'static) -> Worker {
    let (tx, rx) = channel();
    Worker {
        tx,
        _inner: Arc::new(spawn_thread(move || loop {
            thread::sleep(Duration::from_millis(500));

            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    on_exit();
                    break;
                }
                Err(TryRecvError::Empty) => {}
            }
        })),
    }
}
