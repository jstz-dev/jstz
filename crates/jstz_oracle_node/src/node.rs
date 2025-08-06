use jstz_proto::runtime::v2::oracle::OracleRequest;
#[cfg(feature = "v2_runtime")]
use {
    crate::{data_provider::DataProvider, relay::Relay},
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
        let rx: Receiver<OracleRequest> = relay.subscribe()?;
        let provider =
            DataProvider::spawn(public_key, secret_key, node_endpoint, rx).await?;

        Ok(Self {
            _relay: relay,
            _provider: provider,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    fn create_test_keys() -> Result<(PublicKey, SecretKey)> {
        let public_key = PublicKey::from_base58(
            "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        )?;
        let secret_key = SecretKey::from_base58(
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
        )?;
        Ok((public_key, secret_key))
    }

    #[tokio::test]
    async fn spawns_oracle_node_successfully() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let log_path = tmp.path().to_path_buf();

        let (public_key, secret_key) = create_test_keys()?;

        let node_endpoint = "http://localhost:8080".to_string();

        let oracle_node =
            OracleNode::spawn(log_path, public_key, secret_key, node_endpoint).await?;

        assert!(oracle_node._relay.tx.receiver_count() > 0);

        Ok(())
    }

    #[tokio::test]
    async fn oracle_node_drops_gracefully() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let log_path = tmp.path().to_path_buf();

        let (public_key, secret_key) = create_test_keys()?;

        let node_endpoint = "http://localhost:8080".to_string();

        {
            let oracle_node = OracleNode::spawn(
                log_path.clone(),
                public_key,
                secret_key,
                node_endpoint,
            )
            .await?;

            assert!(oracle_node._relay.tx.receiver_count() > 0);
        }

        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    #[tokio::test]
    async fn handles_invalid_log_path() -> Result<()> {
        let (public_key, secret_key) = create_test_keys()?;

        let invalid_log_path = PathBuf::from("/non/existent/path.log");
        let node_endpoint = "http://localhost:8080".to_string();

        let result =
            OracleNode::spawn(invalid_log_path, public_key, secret_key, node_endpoint)
                .await;

        assert!(result.is_err());

        Ok(())
    }
}
