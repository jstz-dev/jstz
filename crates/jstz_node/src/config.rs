use std::path::{Path, PathBuf};

use octez::r#async::endpoint::Endpoint;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct JstzNodeConfig {
    /// The endpoint of the jstz node.
    pub endpoint: Endpoint,
    /// Rollup endpoint.
    pub rollup_endpoint: Endpoint,
    /// The path to the rollup kernel log file.
    pub kernel_log_file: PathBuf,
}

impl JstzNodeConfig {
    pub fn new(
        endpoint: &Endpoint,
        rollup_endpoint: &Endpoint,
        kernel_log_file: &Path,
    ) -> Self {
        Self {
            endpoint: endpoint.clone(),
            rollup_endpoint: rollup_endpoint.clone(),
            kernel_log_file: kernel_log_file.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_config() {
        let config = JstzNodeConfig::new(
            &Endpoint::localhost(8932),
            &Endpoint::localhost(8933),
            Path::new("/tmp/kernel.log"),
        );

        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["endpoint"], "http://localhost:8932");
        assert_eq!(json["rollup_endpoint"], "http://localhost:8933");
        assert_eq!(json["kernel_log_file"], "/tmp/kernel.log");
    }
}
