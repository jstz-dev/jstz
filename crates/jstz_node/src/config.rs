use std::path::{Path, PathBuf};

use jstz_utils::KeyPair;
use octez::r#async::endpoint::Endpoint;
use serde::Serialize;

use crate::RunMode;

#[derive(Clone, Serialize)]
pub struct JstzNodeConfig {
    /// The endpoint of the jstz node.
    pub endpoint: Endpoint,
    /// Rollup endpoint.
    pub rollup_endpoint: Endpoint,
    /// The path to the rollup preimages directory.
    pub rollup_preimages_dir: PathBuf,
    /// The path to the rollup kernel log file.
    pub kernel_log_file: PathBuf,
    #[serde(skip)]
    /// The injector of the operation. Currently, it's used for signing `RevealLargePayload` operation.
    pub injector: KeyPair,
    /// The mode in which the rollup node will run.
    pub mode: RunMode,
    /// Capacity of the operation queue.
    pub capacity: usize,
    /// The path to the sequencer runtime debug log file.
    pub debug_log_file: PathBuf,
    #[cfg(feature = "v2_runtime")]
    /// The Oracle signer used to authenticate valid oracle responses
    pub oracle: Option<KeyPair>,
}

impl JstzNodeConfig {
    /// Create a new JstzNodeConfig.
    ///
    /// If `injector` is not provided, bootstrap1 account will be used as the injector.
    // FIXME: JSTZ-648 turn this into a builder
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        endpoint: &Endpoint,
        rollup_endpoint: &Endpoint,
        rollup_preimages_dir: &Path,
        kernel_log_file: &Path,
        injector: KeyPair,
        mode: RunMode,
        capacity: usize,
        debug_log_file: &Path,
        #[cfg(feature = "v2_runtime")] oracle_key_pair: Option<KeyPair>,
    ) -> Self {
        Self {
            endpoint: endpoint.clone(),
            rollup_endpoint: rollup_endpoint.clone(),
            rollup_preimages_dir: rollup_preimages_dir.to_path_buf(),
            kernel_log_file: kernel_log_file.to_path_buf(),
            injector,
            mode,
            capacity,
            debug_log_file: debug_log_file.to_path_buf(),
            #[cfg(feature = "v2_runtime")]
            oracle: oracle_key_pair,
        }
    }
}

#[cfg(test)]
mod tests {
    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};

    use super::*;

    #[test]
    fn test_serialize_config() {
        let config = JstzNodeConfig::new(
            &Endpoint::localhost(8932),
            &Endpoint::localhost(8933),
            Path::new("/tmp/preimages"),
            Path::new("/tmp/kernel.log"),
            KeyPair(
                PublicKey::from_base58(
                    "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
                )
                .unwrap(),
                SecretKey::from_base58(
                    "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
                )
                .unwrap(),
            ),
            RunMode::Default,
            0,
            Path::new("/tmp/debug.log"),
            #[cfg(feature = "v2_runtime")]
            Some(KeyPair(
                PublicKey::from_base58(
                    "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
                )
                .unwrap(),
                SecretKey::from_base58(
                    "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
                )
                .unwrap(),
            )),
        );

        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["endpoint"], "http://localhost:8932");
        assert_eq!(json["rollup_endpoint"], "http://localhost:8933");
        assert_eq!(json["rollup_preimages_dir"], "/tmp/preimages");
        assert_eq!(json["kernel_log_file"], "/tmp/kernel.log");
        assert_eq!(json["injector"], serde_json::Value::Null);
        assert_eq!(json["debug_log_file"], "/tmp/debug.log");
        #[cfg(feature = "v2_runtime")]
        {
            let oracle_key_pair = &json["oracle"];
            assert!(oracle_key_pair.is_string());
            assert_eq!(
                serde_json::from_value::<String>(oracle_key_pair.clone()).unwrap(),
                "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi"
            );
        }
    }
}
