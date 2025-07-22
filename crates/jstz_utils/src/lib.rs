use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
use serde::Serialize;
pub mod tailed_file;

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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "PublicKey")]
pub struct KeyPair(pub PublicKey, pub SecretKey);

impl From<KeyPair> for PublicKey {
    fn from(value: KeyPair) -> Self {
        value.0
    }
}

// WARNING: Should only be used in tests!
pub mod test_util {
    use super::*;
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
