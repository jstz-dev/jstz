use std::path::{Path, PathBuf};

use octez::r#async::endpoint::Endpoint;

#[derive(Clone)]
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
