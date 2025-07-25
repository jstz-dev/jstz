use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use anyhow::Context;
use jstz_utils::KeyPair;
use octez::r#async::endpoint::Endpoint;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "mode")]
pub enum RunMode {
    Sequencer {
        capacity: usize,
        debug_log_path: PathBuf,
    },
    #[serde(alias = "default")]
    Default,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Default
    }
}

impl Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunMode::Default => write!(f, "default"),
            RunMode::Sequencer { .. } => write!(f, "sequencer"),
        }
    }
}

impl RunMode {
    pub fn new(
        mode_str: Option<&str>,
        capacity: Option<usize>,
        debug_log_path: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        match mode_str.unwrap_or("default") {
            "sequencer" => Ok(RunMode::Sequencer {
                capacity: capacity.unwrap_or(1),
                debug_log_path: debug_log_path.unwrap_or(
                    NamedTempFile::new()
                        .context("failed to create temporary debug log file")?
                        .into_temp_path()
                        .keep()
                        .context("failed to convert temporary debug log file to path")?
                        .to_path_buf(),
                ),
            }),
            "default" => Ok(RunMode::Default),
            v => Err(anyhow::anyhow!("invalid run mode '{v}'")),
        }
    }
}

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
    #[serde(flatten)]
    /// The mode in which the rollup node will run.
    pub mode: RunMode,
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
    ) -> Self {
        Self {
            endpoint: endpoint.clone(),
            rollup_endpoint: rollup_endpoint.clone(),
            rollup_preimages_dir: rollup_preimages_dir.to_path_buf(),
            kernel_log_file: kernel_log_file.to_path_buf(),
            injector,
            mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

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
        );

        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["endpoint"], "http://localhost:8932");
        assert_eq!(json["rollup_endpoint"], "http://localhost:8933");
        assert_eq!(json["rollup_preimages_dir"], "/tmp/preimages");
        assert_eq!(json["kernel_log_file"], "/tmp/kernel.log");
        assert_eq!(json["injector"], serde_json::Value::Null);
        assert_eq!(json["mode"], "default");
        assert_eq!(json["capacity"], serde_json::Value::Null);
        assert_eq!(json["debug_log_path"], serde_json::Value::Null);

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
            RunMode::Sequencer {
                capacity: 123,
                debug_log_path: PathBuf::from_str("/debug/log").unwrap(),
            },
        );

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["mode"], "sequencer");
        assert_eq!(json["capacity"], 123);
        assert_eq!(json["debug_log_path"], "/debug/log");
    }

    #[test]
    fn default_runmode() {
        assert_eq!(RunMode::default(), RunMode::Default);
    }

    #[test]
    fn runmode_to_string() {
        assert_eq!(RunMode::Default.to_string(), "default");
        assert_eq!(
            RunMode::Sequencer {
                capacity: 1,
                debug_log_path: PathBuf::new()
            }
            .to_string(),
            "sequencer"
        );
    }

    #[test]
    fn runmode_new() {
        assert_eq!(RunMode::new(None, None, None).unwrap(), RunMode::Default);
        assert_eq!(
            RunMode::new(Some("default"), None, None).unwrap(),
            RunMode::Default
        );

        let mode = RunMode::new(Some("sequencer"), None, None).unwrap();
        matches!(
            mode,
            RunMode::Sequencer {
                capacity: 1,
                debug_log_path: _
            }
        );

        assert_eq!(
            RunMode::new(
                Some("sequencer"),
                Some(123),
                Some(PathBuf::from_str("/foo/bar").unwrap())
            )
            .unwrap(),
            RunMode::Sequencer {
                capacity: 123,
                debug_log_path: PathBuf::from_str("/foo/bar").unwrap()
            }
        );

        assert_eq!(
            RunMode::new(Some("bad"), None, None)
                .unwrap_err()
                .to_string(),
            "invalid run mode 'bad'"
        );
    }
}
