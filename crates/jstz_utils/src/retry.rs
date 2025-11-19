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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_exponential_backoff() {
        let base = 50;
        let max_attempts = 5;
        let max_delay = Duration::from_secs(8); // allow more growth before capping
        let intervals: Vec<_> =
            exponential_backoff(base, max_attempts, max_delay).collect();
        let expected = vec![
            Duration::from_millis(100),  // 100*2
            Duration::from_millis(5000), // capped from 20_000
            Duration::from_millis(8000), // capped
            Duration::from_millis(8000), // capped
            Duration::from_millis(8000), // capped
        ];
        assert_eq!(intervals, expected);
    }

    #[tokio::test]
    async fn test_retry_async_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let op = {
            let attempts = attempts.clone();
            move || {
                let attempts = attempts.clone();
                async move {
                    let n = attempts.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Err("fail")
                    } else {
                        Ok("success")
                    }
                }
            }
        };
        let should_retry = |_e: &&str| true;
        let backoff = exponential_backoff(1, 5, Duration::from_millis(10));
        let result = retry_async(backoff, op, should_retry).await;
        assert_eq!(result, Ok("success"));
    }

    #[tokio::test]
    async fn test_retry_async_permanent_error() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let op = {
            let attempts = attempts.clone();
            move || {
                let attempts = attempts.clone();
                async move {
                    let n = attempts.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        Err("permanent")
                    } else {
                        Ok("should not reach here")
                    }
                }
            }
        };
        let should_retry = |e: &&str| *e != "permanent";
        let backoff = exponential_backoff(1, 5, Duration::from_millis(10));
        let result = retry_async(backoff, op, should_retry).await;
        assert_eq!(result, Err("permanent"));
    }
}
