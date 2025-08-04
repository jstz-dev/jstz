use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
#[cfg(feature = "v2_runtime")]
use jstz_oracle_node::node::OracleNode;
use serde::Deserialize;

const DEFAULT_JSTZ_NODE_ENDPOINT: &str = "http://127.0.0.1:8933";

#[derive(Debug, Parser)]
#[command(name = "jstz-oracle-node")]
#[command(about = "JSTZ Oracle Node - Provides oracle data for JSTZ rollup")]
struct Args {
    /// Path to the log file
    #[arg(long)]
    log_path: PathBuf,

    /// JSTZ node endpoint
    #[arg(long, default_value = DEFAULT_JSTZ_NODE_ENDPOINT)]
    node_endpoint: String,

    /// Path to file containing key pair (format: "public_key:secret_key")
    #[arg(long)]
    key_file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawKeyPair {
    public_key: String,
    secret_key: String,
}

fn parse_key_file(path: PathBuf) -> anyhow::Result<(PublicKey, SecretKey)> {
    let key_pair = std::fs::read_to_string(path).context("Failed to read key file")?;
    let RawKeyPair {
        public_key,
        secret_key,
    } = serde_json::from_str(&key_pair).map_err(|_| {
        anyhow::anyhow!("Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields")
    })?;

    let public_key = PublicKey::from_base58(&public_key).context("Invalid public key")?;
    let secret_key = SecretKey::from_base58(&secret_key).context("Invalid secret key")?;

    Ok((public_key, secret_key))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let args = Args::parse();

    // Check if log path exists
    if !args.log_path.exists() {
        anyhow::bail!("Log path does not exist: {:?}", args.log_path);
    }

    // Canonicalize log path
    let canonical_log_path = args
        .log_path
        .canonicalize()
        .context("Failed to canonicalize log path")?;

    // Parse key file
    let (public_key, _secret_key) =
        parse_key_file(args.key_file).context("failed to parse key file")?;

    log::info!("Starting JSTZ Oracle Node");
    log::info!(
        "Listening for Oracle request events on: {:?}",
        canonical_log_path
    );
    log::info!("Node endpoint: {}", args.node_endpoint);
    log::info!("Public key: {}", public_key.to_base58());

    // Spawn the oracle node
    #[cfg(feature = "v2_runtime")]
    {
        let _oracle_node = OracleNode::spawn(
            canonical_log_path,
            public_key,
            _secret_key,
            args.node_endpoint,
        )
        .await
        .context("Failed to spawn oracle node")?;

        log::info!("Oracle node started successfully");

        // Keep the node running. The node will keep running until this task is dropped
        // If the Relay or DataProvider dies, the main thread will not be notified. With the
        // current implementation, it is not possible for them to die. This might change in
        // the future.
        tokio::signal::ctrl_c()
            .await
            .context("Failed to wait for Ctrl+C")?;

        log::info!("Shutting down oracle node...");

        Ok(())
    }
    #[cfg(not(feature = "v2_runtime"))]
    {
        anyhow::bail!("Oracle node is not supported in this runtime");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Seek, Write},
        path::PathBuf,
        str::FromStr,
    };

    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use tempfile::NamedTempFile;

    #[test]
    fn parse_key_file() {
        assert_eq!(
            super::parse_key_file(PathBuf::from_str("/foo/bar").unwrap())
                .unwrap_err()
                .to_string(),
            "Failed to read key file"
        );

        let mut tmp_file = NamedTempFile::new().unwrap();
        tmp_file.write_all(b"a:b:c").unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ3",
  "secret_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Invalid public key"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
  "secret_key": "a"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Failed to parse key file. Key file must be JSON with 'public_key' and 'secret_key' fields"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(
                br#"{
  "public_key": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2",
  "secret_key": "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
}"#,
            )
            .unwrap();
        tmp_file.flush().unwrap();
        let (public_key, secret_key) =
            super::parse_key_file(tmp_file.path().to_path_buf()).unwrap();
        assert_eq!(
            public_key,
            PublicKey::from_base58(
                "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
            )
            .unwrap()
        );
        assert_eq!(
            secret_key,
            SecretKey::from_base58(
                "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
            )
            .unwrap()
        );
    }
}
