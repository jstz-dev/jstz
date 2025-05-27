use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{select, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::queue::OperationQueue;

pub struct InboxPoller {
    inner: Option<JoinHandle<()>>,
    kill_sig: CancellationToken,
}

impl InboxPoller {
    pub async fn shut_down(&mut self) {
        self.kill_sig.cancel();
        if let Some(h) = self.inner.take() {
            let _ = h.await;
        }
    }
}

pub fn poll<
    #[cfg(test)] F: std::future::Future<Output = ()> + Send + 'static,
    #[cfg(test)] P: Fn() -> F + Send + 'static,
>(
    _rollup_endpoint: String,
    _queue: Arc<RwLock<OperationQueue>>,
    interval_secs: u64,
    #[cfg(test)] on_poll: P,
) -> InboxPoller {
    let kill_sig = CancellationToken::new();
    let kill_sig_cloned = kill_sig.clone();
    let inner = tokio::spawn(async move {
        loop {
            select! {
                _ = kill_sig_cloned.cancelled() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(interval_secs)) => {
                    #[cfg(test)]
                    on_poll().await;
                    //TODO: Process inbox messages.
                }
            }
        }
    });
    InboxPoller {
        inner: Some(inner),
        kill_sig,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex, RwLock},
        time::Duration,
    };

    use crate::sequencer::{inbox::poll, queue::OperationQueue};

    fn make_on_poll(
        min_execution_time_secs: u64,
    ) -> (
        Arc<Mutex<i32>>,
        impl Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
    ) {
        let counter = Arc::new(Mutex::new(0));
        let cloned = counter.clone();
        let on_poll = move || {
            let cloned = cloned.clone();
            Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(min_execution_time_secs)).await;
                let mut value = cloned.lock().unwrap();
                *value += 1;
            })
        } as Pin<Box<dyn Future<Output = ()> + Send>>;
        (counter, on_poll)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn executes_polling() {
        let poll_interval_secs = 1;
        let poll_execution_secs = 0;
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (poll_count, on_poll) = make_on_poll(poll_execution_secs);
        let _ = poll(
            "rollup_endpoint".to_string(),
            q,
            poll_interval_secs,
            on_poll,
        );
        // After 4 seconds, polls at least 3 times
        tokio::time::sleep(Duration::from_secs(4)).await;
        assert_eq!(*poll_count.lock().unwrap(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn polling_does_not_overlap() {
        let poll_interval_secs = 1;
        let poll_execution_secs = 2;
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (poll_count, on_poll) = make_on_poll(poll_execution_secs);
        let _ = poll(
            "rollup_endpoint".to_string(),
            q,
            poll_interval_secs,
            on_poll,
        );
        // After 4 seconds, polls no more than once.
        tokio::time::sleep(Duration::from_secs(4)).await;
        assert!(*poll_count.lock().unwrap() <= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancels_polling() {
        let poll_interval_secs = 1;
        let poll_execution_secs = 0;
        let q = Arc::new(RwLock::new(OperationQueue::new(0)));
        let (poll_count, on_poll) = make_on_poll(poll_execution_secs);
        let mut poller = poll(
            "rollup_endpoint".to_string(),
            q,
            poll_interval_secs,
            on_poll,
        );
        poller.shut_down().await;
        assert_eq!(*poll_count.lock().unwrap(), 0);
    }
}
