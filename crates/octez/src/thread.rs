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
    thread_handle: JoinHandle<Result<()>>,
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
            thread_handle,
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
            thread_handle,
        }
    }

    pub fn is_running(&self) -> bool {
        !self.thread_handle.is_finished()
    }

    pub fn shutdown(self) -> Result<()> {
        self.shutdown_tx.send(())?;
        match self.thread_handle.join() {
            Ok(result) => result?,
            Err(_) => {
                // thread panicked
            }
        };
        Ok(())
    }

    pub fn join(threads: Vec<Self>) -> Result<()> {
        let mut signals = Signals::new([SIGINT, SIGTERM])?;

        // Loop until 1 of the threads fails
        'main_loop: loop {
            for thread in threads.iter() {
                if !thread.is_running() {
                    break 'main_loop;
                }
            }

            for signal in signals.pending() {
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
            if thread.is_running() {
                thread.shutdown()?;
            }
        }

        Ok(())
    }
}
