use crate::unused_port;
use anyhow::Result;
use std::{
    fmt::{self, Display, Formatter},
    path::PathBuf,
    str::FromStr,
};

use super::endpoint::Endpoint;

const DEFAULT_NETWORK: &str = "sandbox";
const DEFAULT_BINARY_PATH: &str = "octez-node";
const LOCAL_ADDRESS: &str = "127.0.0.1";

#[derive(Clone, PartialEq, Debug)]
pub enum OctezNodeHistoryMode {
    Archive,
    // The numerical value represents additional cycles preserved. 0 is acceptable.
    Full(u8),
    Rolling(u8),
}

impl Display for OctezNodeHistoryMode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Archive => write!(f, "archive"),
            Self::Full(v) => write!(f, "full:{}", v),
            Self::Rolling(v) => write!(f, "rolling:{}", v),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct OctezNodeRunOptions {
    synchonisation_threshold: u8,
    network: String,
    history_mode: Option<OctezNodeHistoryMode>,
}

impl Display for OctezNodeRunOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut s = vec![];
        s.push(format!(
            "--synchronisation-threshold {}",
            &self.synchonisation_threshold
        ));
        s.push(format!("--network {}", &self.network));
        if let Some(v) = &self.history_mode {
            s.push(format!("--history-mode {}", v));
        }
        let line = s.join(" ");
        write!(f, "{}", line)
    }
}

impl Default for OctezNodeRunOptions {
    fn default() -> Self {
        Self {
            network: DEFAULT_NETWORK.to_owned(),
            synchonisation_threshold: 0,
            history_mode: None,
        }
    }
}

#[derive(Default)]
pub struct OctezNodeRunOptionsBuilder {
    synchonisation_threshold: Option<u8>,
    network: Option<String>,
    history_mode: Option<OctezNodeHistoryMode>,
}

impl OctezNodeRunOptionsBuilder {
    pub fn new() -> Self {
        OctezNodeRunOptionsBuilder {
            ..Default::default()
        }
    }

    pub fn set_synchronisation_threshold(&mut self, threshold: u8) -> &mut Self {
        self.synchonisation_threshold.replace(threshold);
        self
    }

    pub fn set_network(&mut self, network: &str) -> &mut Self {
        self.network.replace(network.to_owned());
        self
    }

    pub fn set_history_mode(&mut self, mode: OctezNodeHistoryMode) -> &mut Self {
        self.history_mode.replace(mode);
        self
    }

    pub fn build(&mut self) -> OctezNodeRunOptions {
        OctezNodeRunOptions {
            synchonisation_threshold: self
                .synchonisation_threshold
                .take()
                .unwrap_or_default(),
            network: self.network.take().unwrap_or(DEFAULT_NETWORK.to_owned()),
            history_mode: self.history_mode.take(),
        }
    }
}

#[derive(Clone)]
pub struct OctezNodeConfig {
    /// Path to the octez node binary.
    pub binary_path: PathBuf,
    /// Path to the directory where the node keeps data.
    pub data_dir: Option<PathBuf>,
    /// Name of the tezos network that the node instance runs on.
    pub network: String,
    /// HTTP endpoint of the node RPC interface, e.g. 'localhost:8732'
    pub rpc_endpoint: Endpoint,
    // TCP address and port at for p2p which this instance can be reached
    pub p2p_address: Endpoint,
    /// Path to the file that keeps octez node logs.
    pub log_file: Option<PathBuf>,
    /// Run options for octez node.
    pub run_options: OctezNodeRunOptions,
}

#[derive(Default)]
pub struct OctezNodeConfigBuilder {
    binary_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    network: Option<String>,
    rpc_endpoint: Option<Endpoint>,
    p2p_endpoint: Option<Endpoint>,
    log_file: Option<PathBuf>,
    run_options: Option<OctezNodeRunOptions>,
}

impl OctezNodeConfigBuilder {
    pub fn new() -> Self {
        OctezNodeConfigBuilder::default()
    }

    /// Sets the path to the octez node binary.
    pub fn set_binary_path(&mut self, path: &str) -> &mut Self {
        self.binary_path = Some(PathBuf::from(path));
        self
    }

    /// Sets the path to the directory where the node keeps data.
    pub fn set_data_dir(&mut self, path: &str) -> &mut Self {
        self.data_dir = Some(PathBuf::from(path));
        self
    }

    /// Sets the name of the tezos network that the node instance runs on.
    pub fn set_network(&mut self, network: &str) -> &mut Self {
        self.network = Some(network.to_owned());
        self
    }

    /// Sets the HTTP endpoint of the node RPC interface, e.g. 'localhost:8732'
    pub fn set_rpc_endpoint(&mut self, endpoint: &Endpoint) -> &mut Self {
        self.rpc_endpoint = Some(endpoint.to_owned());
        self
    }

    pub fn set_p2p_endpoint(&mut self, endpoint: &Endpoint) -> &mut Self {
        self.p2p_endpoint = Some(endpoint.to_owned());
        self
    }

    /// Sets the path to the file that keeps octez node logs.
    pub fn set_log_file(&mut self, path: &str) -> &mut Self {
        self.log_file = Some(PathBuf::from(path));
        self
    }

    /// Sets run options for octez node.
    pub fn set_run_options(&mut self, options: &OctezNodeRunOptions) -> &mut Self {
        self.run_options.replace(options.clone());
        self
    }

    /// Builds a config set based on values collected.
    pub fn build(&mut self) -> Result<OctezNodeConfig> {
        Ok(OctezNodeConfig {
            binary_path: self
                .binary_path
                .take()
                .unwrap_or(PathBuf::from(DEFAULT_BINARY_PATH)),
            data_dir: self.data_dir.take(),
            network: self.network.take().unwrap_or(DEFAULT_NETWORK.to_owned()),
            rpc_endpoint: self
                .rpc_endpoint
                .take()
                .unwrap_or(Endpoint::localhost(unused_port())),
            p2p_address: self.p2p_endpoint.take().unwrap_or(
                Endpoint::try_from(
                    http::Uri::from_str(&format!("{}:{}", LOCAL_ADDRESS, unused_port()))
                        .unwrap(),
                )
                .unwrap(),
            ),
            log_file: self.log_file.take(),
            run_options: self.run_options.take().unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn config_builder() {
        let mut run_options_builder = OctezNodeRunOptionsBuilder::new();
        let run_options = run_options_builder.set_network("sandbox").build();
        let config = OctezNodeConfigBuilder::new()
            .set_binary_path("/tmp/binary")
            .set_data_dir("/tmp/something")
            .set_network("network")
            .set_rpc_endpoint(&Endpoint::localhost(8888))
            .set_log_file("/log_file")
            .set_run_options(&run_options)
            .build()
            .unwrap();
        assert_eq!(config.binary_path, PathBuf::from("/tmp/binary"));
        assert_eq!(config.data_dir, Some(PathBuf::from("/tmp/something")));
        assert_eq!(config.network, "network".to_owned());
        assert_eq!(config.rpc_endpoint, Endpoint::localhost(8888));
        assert_eq!(config.log_file, Some(PathBuf::from("/log_file")));
        assert_eq!(config.run_options, run_options);
    }

    #[test]
    fn config_builder_default() {
        let config = OctezNodeConfigBuilder::new().build().unwrap();
        assert_eq!(config.binary_path, PathBuf::from(DEFAULT_BINARY_PATH));
        assert_eq!(config.network, DEFAULT_NETWORK.to_owned());
        assert_eq!(config.run_options, OctezNodeRunOptions::default());
    }

    #[test]
    fn run_option_builder() {
        let mut run_options_builder = OctezNodeRunOptionsBuilder::new();
        let run_options = run_options_builder
            .set_network("foo")
            .set_history_mode(OctezNodeHistoryMode::Full(5))
            .set_synchronisation_threshold(3)
            .build();
        assert_eq!(
            run_options.history_mode.unwrap(),
            OctezNodeHistoryMode::Full(5)
        );
        assert_eq!(run_options.network, "foo");
        assert_eq!(run_options.synchonisation_threshold, 3);
    }

    #[test]
    fn run_option_builder_default() {
        let mut run_options_builder = OctezNodeRunOptionsBuilder::new();
        let run_options = run_options_builder.build();
        assert!(run_options.history_mode.is_none());
        assert_eq!(run_options.network, "sandbox");
        assert_eq!(run_options.synchonisation_threshold, 0);
    }

    #[test]
    fn run_option_default() {
        let run_options = OctezNodeRunOptions::default();
        assert!(run_options.history_mode.is_none());
        assert_eq!(run_options.network, "sandbox");
        assert_eq!(run_options.synchonisation_threshold, 0);
    }

    #[test]
    fn run_option_to_string() {
        let mut run_options_builder = OctezNodeRunOptionsBuilder::new();
        let run_options = run_options_builder
            .set_network("foo")
            .set_history_mode(OctezNodeHistoryMode::Full(5))
            .set_synchronisation_threshold(3)
            .build()
            .to_string();
        assert_eq!(
            run_options,
            "--synchronisation-threshold 3 --network foo --history-mode full:5"
        );

        // No history mode
        let run_options = run_options_builder
            .set_network("foo")
            .set_synchronisation_threshold(3)
            .build()
            .to_string();
        assert_eq!(run_options, "--synchronisation-threshold 3 --network foo");
    }

    #[test]
    fn history_mode_to_string() {
        assert_eq!(OctezNodeHistoryMode::Archive.to_string(), "archive");
        assert_eq!(OctezNodeHistoryMode::Full(2).to_string(), "full:2");
        assert_eq!(OctezNodeHistoryMode::Rolling(1).to_string(), "rolling:1");
    }
}
