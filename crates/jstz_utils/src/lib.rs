pub mod event_stream;
pub mod filtered_log_stream;
#[cfg(feature = "inbox_builder")]
pub mod inbox_builder;
pub mod key_pair;
pub mod retry;
pub mod tailed_file;
pub use key_pair::KeyPair;

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
#[cfg(any(test, feature = "test_utils"))]
pub mod test_util {
    use crate::key_pair::KeyPair;
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use std::{path::PathBuf, time::Duration};
    use tokio::fs::OpenOptions;
    use tokio::io::AsyncWriteExt;

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

    pub fn alice_keys() -> KeyPair {
        let alice_sk = SecretKey::from_base58(
            "edsk38mmuJeEfSYGiwLE1qHr16BPYKMT5Gg1mULT7dNUtg3ti4De3a",
        )
        .unwrap();
        let alice_pk = PublicKey::from_base58(
            "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
        )
        .unwrap();
        KeyPair(alice_pk, alice_sk)
    }

    pub fn bob_keys() -> KeyPair {
        let bob_sk = SecretKey::from_base58(
            "edsk3eA4FyZDnDSC2pzEh4kwnaLLknvdikvRuXZAV4T4pWMVd6GUyS",
        )
        .unwrap();
        let bob_pk = PublicKey::from_base58(
            "edpkusQcxu7Zv33x1p54p62UgzcawjBRSdEFJbPKEtjQ1h1TaFV3U5",
        )
        .unwrap();
        KeyPair(bob_pk, bob_sk)
    }

    pub async fn append_async(
        path: PathBuf,
        line: String,
        delay_ms: u64,
    ) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        let mut file = OpenOptions::new().append(true).open(&path).await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
        Ok(())
    }

    use std::sync::{Arc, Mutex};
    use tezos_smart_rollup_mock::DebugSink;

    #[derive(Clone, Default)]
    pub struct DebugLogSink {
        pub inner: Arc<Mutex<Vec<u8>>>,
    }

    impl DebugSink for DebugLogSink {
        fn write_all(&mut self, buffer: &[u8]) -> std::io::Result<()> {
            self.inner.lock().unwrap().extend_from_slice(buffer);
            Ok(())
        }
    }

    impl DebugLogSink {
        pub fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(vec![])),
            }
        }

        pub fn content(&self) -> Arc<Mutex<Vec<u8>>> {
            self.inner.clone()
        }

        pub fn str_content(&self) -> String {
            let buf = self.inner.lock().unwrap();
            String::from_utf8(buf.to_vec()).unwrap()
        }

        pub fn lines(&self) -> Vec<String> {
            let str_content = self.str_content();
            str_content.split("\n").map(|s| s.to_string()).collect()
        }
    }
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
