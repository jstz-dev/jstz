use anyhow;
use async_dropper::AsyncDrop;
use async_trait::async_trait;
use futures_util::{
    task::{Context, Poll},
    Future,
};
use tokio::runtime::Handle;
use tokio::task::JoinHandle as tokioJoinHandle;

use std::pin::Pin;
use std::sync::Arc;
use tokio::signal::unix::Signal;
use tokio::sync::{mpsc, Mutex};

#[derive(Default)]
struct JoinHandleWrapper {
    handle: Option<tokioJoinHandle<()>>,
    dropped: bool,
}

impl JoinHandleWrapper {
    pub fn is_finished(&self) -> bool {
        match &self.handle {
            Some(v) => v.is_finished(),
            None => true,
        }
    }

    pub fn abort(&self) {
        if let Some(v) = &self.handle {
            v.abort()
        }
    }
}

impl Drop for JoinHandleWrapper {
    fn drop(&mut self) {
        if !self.dropped {
            // Prevent the copy `this` to drop again
            self.dropped = true;
            let mut this = std::mem::take(self);
            // Prevent the original `self` to drop again
            self.dropped = true;
            tokio::task::block_in_place(move || {
                Handle::current().block_on(async move {
                    this.async_drop().await;
                });
            });
        }
    }
}

// On drop, send shutdown signal and wait for the task to terminate
#[async_trait]
impl AsyncDrop for JoinHandleWrapper {
    async fn async_drop(&mut self) {
        if let Some(v) = &mut self.handle {
            v.abort();
            let _ = v.await;
        }
    }
}

#[derive(Default, Clone)]
pub struct JoinHandle {
    handle: Option<Arc<JoinHandleWrapper>>,
    signal_tx: Option<mpsc::Sender<Signal>>,
    execution_result: Arc<Mutex<Option<anyhow::Result<()>>>>,
}

impl JoinHandle {
    /// Creates a new join handle from a task.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use jstzd::task::joinhandle::JoinHandle;
    /// use tokio::signal::unix::SignalKind;
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, _) = mpsc::channel(1);
    ///     let handle = JoinHandle::new(
    ///         async move {
    ///             tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    ///             anyhow::Ok(())
    ///         },
    ///         tx.clone(),
    ///     );
    /// }
    /// ```
    pub fn new<F>(task: F, signal_tx: mpsc::Sender<Signal>) -> Self
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let mutex = Arc::new(Mutex::new(None));
        let copy1 = mutex.clone();
        let handle = tokio::spawn(async move {
            let execution_result = task.await;
            let mut result_container = copy1.lock().await;
            result_container.replace(execution_result);
        });

        JoinHandle {
            handle: Some(Arc::new(JoinHandleWrapper {
                handle: Some(handle),
                dropped: false,
            })),
            signal_tx: Some(signal_tx),
            execution_result: mutex,
        }
    }

    pub async fn wait(&mut self) -> anyhow::Result<()> {
        loop {
            if let Ok(mut result_container) = self.execution_result.try_lock() {
                let finished = match self.handle.as_ref() {
                    Some(handle) => handle.is_finished(),
                    None => return anyhow::Ok(()),
                };
                if result_container.is_some() {
                    return result_container.take().unwrap();
                } else if finished {
                    // Since the result of the actual task is stored into the container mutex
                    // before the wrapper handle returns, here it's assumed that if the wrapper
                    // task is finished but there is no execution result, the task must have
                    // panicked and failed the wrapper handle.
                    return Err(anyhow::anyhow!("task panicked"));
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs_f32(0.05)).await;
        }
    }

    /// This function will signal the running task with the specified signal
    /// This is a non-blocking operation
    pub fn signal(&self, signal: Signal) -> anyhow::Result<()> {
        if let Some(tx) = &self.signal_tx {
            let copy = tx.clone();
            tokio::spawn(async move {
                let _ = copy.send(signal).await;
            });
        }
        anyhow::Ok(())
    }

    pub async fn abort(&mut self) {
        if let Some(handle) = self.handle.as_ref() {
            handle.abort();
        }
        let _ = self.wait().await;
    }
}

impl Future for JoinHandle {
    type Output = anyhow::Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return match &self.execution_result.try_lock() {
            Ok(result_container) => {
                let finished = self.handle.as_ref().unwrap().is_finished();
                if result_container.is_some() {
                    let result = result_container.as_ref().unwrap();
                    if result.is_ok() {
                        Poll::Ready(anyhow::Ok(()))
                    } else {
                        // The error is "swallowed" here by keeping its string form only
                        // because anyhow::error cannot be copied
                        Poll::Ready(Err(anyhow::anyhow!(result
                            .as_ref()
                            .unwrap_err()
                            .to_string())))
                    }
                } else if finished {
                    // Since the result of the actual task is stored into the container mutex
                    // before the wrapper handle returns, here it's assumed that if the wrapper
                    // task is finished but there is no execution result, the task must have
                    // panicked and failed the wrapper handle.
                    Poll::Ready(Err(anyhow::anyhow!("task panicked")))
                } else {
                    let waker = cx.waker().clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs_f32(0.3)).await;
                        waker.wake();
                    });
                    Poll::Pending
                }
            }
            Err(_) => Poll::Pending,
        };
    }
}

#[cfg(test)]
mod test_joinhandle {
    use tokio::signal::unix::{signal, SignalKind};

    use super::JoinHandle;
    use std::panic;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::{mpsc, Mutex};

    #[tokio::test(flavor = "multi_thread")]
    async fn wait_ok() {
        let num = Arc::new(Mutex::new(0));
        let copy = num.clone();
        let duration = 0.5;
        let (tx, _) = mpsc::channel(1);
        let start_time = Instant::now();
        let mut handle = JoinHandle::new(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
                let mut v = copy.lock().await;
                *v += 1;
                anyhow::Ok(())
            },
            tx,
        );
        assert!((handle.wait().await).is_ok());
        assert!((start_time.elapsed().as_secs_f32() - duration).abs() < 0.05);
        let v = num.lock().await;
        assert_eq!(*v, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wait_err() {
        let duration = 0.1;
        let start_time = Instant::now();
        let (tx, _) = mpsc::channel(1);
        let mut handle = JoinHandle::new(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
                panic!();
            },
            tx,
        );
        assert!((handle.wait().await).is_err());
        assert!((start_time.elapsed().as_secs_f32() - duration).abs() < 0.1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn signal_send() {
        let duration = 0.5;
        let num = Arc::new(Mutex::new(0));
        let copy = num.clone();
        let (tx, mut rx) = mpsc::channel(1);
        let mut handle = JoinHandle::new(
            async move {
                for _ in 0..5 {
                    if rx.try_recv().is_ok() {
                        let mut v = copy.lock().await;
                        *v += 1;
                        return anyhow::Ok(());
                    }
                    tokio::time::sleep(std::time::Duration::from_secs_f32(duration))
                        .await;
                }
                Err(anyhow::anyhow!("should not reach here"))
            },
            tx.clone(),
        );

        let handle_copy = handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
            let s = signal(SignalKind::terminate()).unwrap();
            assert!(handle_copy.signal(s).is_ok());
        });

        assert!((handle.wait().await).is_ok());
        let v = num.lock().await;
        assert_eq!(*v, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn abort_test() {
        let num = Arc::new(Mutex::new(0));
        let num_copy = num.clone();
        let duration = 0.3;
        let (tx, _) = mpsc::channel(1);
        let start_time = Instant::now();
        let mut handle = JoinHandle::new(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
                let mut v = num_copy.lock().await;
                *v += 1;
                Err(anyhow::anyhow!("should not reach here"))
            },
            tx.clone(),
        );
        let mut handle_copy = handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
            handle_copy.abort().await;
        });

        assert_eq!(
            (handle.wait().await).unwrap_err().to_string(),
            "task panicked"
        );
        assert!(start_time.elapsed().as_secs_f32() < 1.0);
        let v = num.lock().await;
        assert_eq!(*v, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn future_implementation() {
        let duration = 0.5;
        let start_time = Instant::now();
        let (tx, _) = mpsc::channel(1);
        let handle = JoinHandle::new(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
                panic!();
            },
            tx,
        );

        assert!((handle.await).is_err());
        // Error margin is larger here because of the polling period in the future implementation
        assert!((start_time.elapsed().as_secs_f32() - duration).abs() < 0.5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn drop_test() {
        let num = Arc::new(Mutex::new(0));
        let num_copy = num.clone();
        let duration = 0.3;
        let (tx, _) = mpsc::channel(1);
        let start_time = Instant::now();
        let handle = JoinHandle::new(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
                let mut v = num_copy.lock().await;
                *v += 1;
                Err(anyhow::anyhow!("should not reach here"))
            },
            tx.clone(),
        );
        tokio::time::sleep(std::time::Duration::from_secs_f32(duration)).await;
        drop(handle);
        assert!(start_time.elapsed().as_secs_f32() < 0.5);
        let v = num.lock().await;
        assert_eq!(*v, 0);
    }
}
