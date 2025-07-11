use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use jstz_node::{RunMode, RunOptions};
use jstz_utils::KeyPair;
use tempfile::NamedTempFile;

const DEFAULT_ROLLUP_NODE_RPC_ADDR: &str = "127.0.0.1";
const DEFAULT_ROLLUP_RPC_PORT: u16 = 8932;
const DEFAULT_KERNEL_LOG_PATH: &str = "logs/kernel.log";

// Endpoint defaults for the `jstz-node`
const DEFAULT_JSTZ_NODE_ADDR: &str = "127.0.0.1";
const DEFAULT_JSTZ_NODE_PORT: u16 = 8933;
const DEFAULT_RUN_MODE: &str = "default";
const DEFAULT_QUEUE_CAPACITY: usize = 1024;

#[derive(Debug, Parser)]
enum Command {
    Run(Args),
    Spec {
        /// Output path of the OpenAPI spec
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = DEFAULT_JSTZ_NODE_ADDR)]
    addr: String,

    #[arg(long, default_value_t = DEFAULT_JSTZ_NODE_PORT)]
    port: u16,

    #[arg(long, default_value = DEFAULT_ROLLUP_NODE_RPC_ADDR)]
    rollup_node_rpc_addr: String,

    #[arg(long, default_value_t = DEFAULT_ROLLUP_RPC_PORT)]
    rollup_node_rpc_port: u16,

    #[arg(short, long)]
    rollup_endpoint: Option<String>,

    #[arg(long, default_value = DEFAULT_KERNEL_LOG_PATH)]
    kernel_log_path: PathBuf,

    #[arg(long)]
    preimages_dir: PathBuf,

    #[arg(long, default_value = DEFAULT_RUN_MODE)]
    mode: RunMode,

    #[arg(long, default_value_t = DEFAULT_QUEUE_CAPACITY)]
    capacity: usize,

    #[arg(long)]
    debug_log_path: Option<PathBuf>,

    /// Path to file containing injector key pair (format: "public_key:secret_key")
    #[arg(long)]
    injector_key_file: PathBuf,

    /// Path to file containing oracle key pair for DataProvider (format: "public_key:secret_key")
    #[arg(long)]
    oracle_key_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    match Command::parse() {
        Command::Run(args) => {
            let rollup_endpoint = args.rollup_endpoint.unwrap_or(format!(
                "http://{}:{}",
                args.rollup_node_rpc_addr, args.rollup_node_rpc_port
            ));

            // Parse oracle key if provided
            #[cfg(feature = "v2_runtime")]
            let oracle_key_pair = if let Some(oracle_key_file) = args.oracle_key_file {
                Some(
                    parse_key_file(oracle_key_file)
                        .context("failed to parse oracle key file")?,
                )
            } else {
                None
            };

            jstz_node::run(RunOptions {
                addr: args.addr,
                port: args.port,
                rollup_endpoint,
                rollup_preimages_dir: args.preimages_dir,
                kernel_log_path: args.kernel_log_path,
                injector: parse_key_file(args.injector_key_file)
                    .context("failed to parse injector key file")?,
                mode: args.mode,
                capacity: args.capacity,
                debug_log_path: args.debug_log_path.unwrap_or(
                    NamedTempFile::new()
                        .context("failed to create temporary debug log file")?
                        .into_temp_path()
                        .keep()
                        .context("failed to convert temporary debug log file to path")?
                        .to_path_buf(),
                ),
                #[cfg(feature = "v2_runtime")]
                oracle_key_pair,
            })
            .await
        }
        Command::Spec { out } => {
            let spec = jstz_node::openapi_json_raw()?;
            match out {
                Some(out) => std::fs::write(out, spec)?,
                None => println!("{}", spec),
            }
            Ok(())
        }
    }
}

fn parse_key_file(path: PathBuf) -> anyhow::Result<KeyPair> {
    let key_pair = std::fs::read_to_string(path).context("Failed to read key file")?;
    let parts: Vec<&str> = key_pair.trim().split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Key pair must be in format 'public_key:secret_key'");
    }
    let public_key = jstz_crypto::public_key::PublicKey::from_base58(parts[0])
        .context("Invalid public key")?;
    let secret_key = jstz_crypto::secret_key::SecretKey::from_base58(parts[1])
        .context("Invalid secret key")?;
    Ok(KeyPair(public_key, secret_key))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Seek, Write},
        path::PathBuf,
        str::FromStr,
    };

    use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
    use jstz_utils::KeyPair;
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
            "Key pair must be in format 'public_key:secret_key'"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(b"edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ3:edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2")
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
            .write_all(b"edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2:a")
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf())
                .unwrap_err()
                .to_string(),
            "Invalid secret key"
        );

        tmp_file.rewind().unwrap();
        tmp_file
            .write_all(b"edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6\n")
            .unwrap();
        tmp_file.flush().unwrap();
        assert_eq!(
            super::parse_key_file(tmp_file.path().to_path_buf()).unwrap(),
            KeyPair(
                PublicKey::from_base58(
                    "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"
                )
                .unwrap(),
                SecretKey::from_base58(
                    "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
                )
                .unwrap()
            )
        );
    }
}
