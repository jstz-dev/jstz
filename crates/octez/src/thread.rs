use std::{
    process::Child,
    sync::mpsc::{self, Sender},
    thread::{self, sleep, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

pub struct OctezThread {
    shutdown_tx: Sender<()>,
    thread_handle: Option<JoinHandle<Result<()>>>,
}

impl OctezThread {
    pub fn new<I, F>(x: I, f: F) -> Self
    where
        F: Fn(&I) -> Result<()> + Send + 'static,
        I: Send + 'static,
    {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let thread_handle: JoinHandle<Result<()>> = thread::spawn(move || {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                f(&x)?;

                sleep(Duration::from_secs(1));
            }

            Ok(())
        });

        Self {
            shutdown_tx,
            thread_handle: Some(thread_handle),
        }
    }

    pub fn from_child(mut child: Child) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let thread_handle: JoinHandle<Result<()>> = thread::spawn(move || {
            loop {
                if child.try_wait()?.is_some() {
                    break;
                }

                if shutdown_rx.try_recv().is_ok() {
                    child.kill()?;
                    break;
                }

                sleep(Duration::from_secs(1));
            }

            Ok(())
        });

        Self {
            shutdown_tx,
            thread_handle: Some(thread_handle),
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some(thread_handle) = &self.thread_handle {
            !thread_handle.is_finished()
        } else {
            false
        }
    }

    pub fn shutdown(&mut self) -> Result<()> {
        if self.is_running() {
            let _ = self.shutdown_tx.send(());
            self.thread_handle.take().unwrap().join().unwrap()?;
        }
        Ok(())
    }

    pub fn join(threads: &mut Vec<&mut Self>) -> Result<()> {
        let mut signals = Signals::new([SIGINT, SIGTERM])?;

        // Loop until 1 of the threads fails
        'main_loop: loop {
            for thread in threads.iter() {
                if !thread.is_running() {
                    break 'main_loop;
                }
            }

            if let Some(signal) = signals.pending().next() {
                match signal {
                    SIGINT | SIGTERM => {
                        println!("Received signal {:?}, shutting down...", signal);
                        break 'main_loop;
                    }
                    _ => unreachable!(),
                }
            }
        }

        // Shutdown all running threads
        for thread in threads {
            thread.shutdown()?;
        }

        Ok(())
    }
}

impl Drop for OctezThread {
    fn drop(&mut self) {
        self.shutdown().unwrap();
    }
}
