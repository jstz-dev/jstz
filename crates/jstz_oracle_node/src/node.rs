#[cfg(feature = "v2_runtime")]
use {
    crate::{data_provider::DataProvider, relay::Relay, request::OracleRequest},
    anyhow::Result,
    jstz_crypto::{public_key::PublicKey, secret_key::SecretKey},
    std::path::PathBuf,
    tokio::sync::broadcast::Receiver,
};

/// Keeps the background tasks alive.
///
/// Dropping `OracleNode` cancels both the relay and the data‑provider tasks
/// (because their `JoinHandle`s are dropped).
#[cfg(feature = "v2_runtime")]
pub struct OracleNode {
    /// Owns the relay so its task isn't dropped.
    _relay: Relay,
    /// Ditto for the data‑provider.
    _provider: DataProvider,
}

#[cfg(feature = "v2_runtime")]
impl OracleNode {
    pub async fn spawn(
        log_path: PathBuf,
        public_key: PublicKey,
        secret_key: SecretKey,
        node_endpoint: String,
    ) -> Result<Self> {
        let relay = Relay::spawn(log_path).await?;
        let rx: Receiver<OracleRequest> = relay.subscribe();
        let provider =
            DataProvider::spawn(public_key, secret_key, node_endpoint, rx).await?;

        Ok(Self {
            _relay: relay,
            _provider: provider,
        })
    }
}
