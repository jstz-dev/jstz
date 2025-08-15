use std::{future::Future, time::Duration};

use tokio_retry2::{strategy::ExponentialBackoff, Retry, RetryError};

pub fn exponential_backoff(
    base: u64,
    max_attempts: usize,
    max_delay: Duration,
) -> impl Iterator<Item = Duration> {
    ExponentialBackoff::from_millis(base)
        .factor(2)
        .max_delay(max_delay)
        .take(max_attempts)
}

pub async fn retry_async<F, Fut, T, E, C>(
    backoff: impl IntoIterator<Item = Duration>,
    mut op: F,
    should_retry: C,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    C: Fn(&E) -> bool + Copy,
{
    let action = || {
        let fut = op();

        async move {
            match fut.await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if should_retry(&e) {
                        Err(RetryError::transient(e))
                    } else {
                        Err(RetryError::permanent(e))
                    }
                }
            }
        }
    };

    Retry::spawn(backoff, action).await
}
