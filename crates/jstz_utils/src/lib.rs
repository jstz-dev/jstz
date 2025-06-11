<<<<<<< HEAD
=======
use std::sync::LazyLock;
pub mod tailed_file;

>>>>>>> 81a3c8e4 (feat(oracle): filetered log stream)
pub async fn poll<'a, F, T>(
    max_attempts: u16,
    interval_ms: u64,
    f: impl Fn() -> F,
) -> Option<T>
where
    F: std::future::Future<Output = Option<T>> + Send + 'a,
{
    let duration = tokio::time::Duration::from_millis(interval_ms);
    for _ in 0..max_attempts {
        tokio::time::sleep(duration).await;
        if let Some(v) = f().await {
            return Some(v);
        }
    }
    None
}

// WARNING: Should only be used in tests!
pub mod test_util {
    // Global tokio instance to prevent races among v2 runtime tests
    pub static TOKIO: std::sync::LazyLock<tokio::runtime::Runtime> =
        std::sync::LazyLock::new(|| {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
        });

    pub static TOKIO_MULTI_THREAD: std::sync::LazyLock<tokio::runtime::Runtime> =
        std::sync::LazyLock::new(|| {
            tokio::runtime::Builder::new_multi_thread().build().unwrap()
        });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn poll() {
        async fn check(locked: Arc<Mutex<i32>>, result: bool) -> Option<bool> {
            let mut v = locked.lock().await;
            if *v == 5 {
                return Some(result);
            }
            *v += 1;
            None
        }

        // poll till the end and get a positive result
        let locked = Arc::new(Mutex::new(1));
        assert!(
            super::poll(5, 1, || async { check(locked.clone(), true).await })
                .await
                .unwrap()
        );

        // poll till the end and get a negative result
        let locked = Arc::new(Mutex::new(1));
        assert!(
            !super::poll(5, 1, || async { check(locked.clone(), false).await })
                .await
                .unwrap()
        );

        // not waiting long enough
        let locked = Arc::new(Mutex::new(1));
        assert!(
            super::poll(2, 1, || async { check(locked.clone(), true).await })
                .await
                .is_none()
        );
    }
}
