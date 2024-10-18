use crate::jstzd::DEFAULT_RPC_ENDPOINT;

use super::{directory::Directory, endpoint::Endpoint};
use anyhow::{anyhow, bail, Result};
use http::Uri;
use jstz_crypto::public_key::PublicKey;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::secret_key::SecretKey;
use std::path::Path;
use std::{ffi::OsStr, fmt, path::PathBuf, str::FromStr};
use tempfile::tempdir;
use tokio::process::Command;

const DEFAULT_BINARY_PATH: &str = "octez-client";

type StdOut = String;

#[derive(Default)]
pub struct OctezClientBuilder {
    // if None, use the binary in $PATH
    binary_path: Option<PathBuf>,
    // if None, use temp directory
    base_dir: Option<PathBuf>,
    // if None, use localhost:8732 (the default endpoint for octez-node)
    endpoint: Option<Endpoint>,
    disable_unsafe_disclaimer: bool,
}

impl OctezClientBuilder {
    pub fn new() -> Self {
        OctezClientBuilder::default()
    }

    pub fn set_binary_path(mut self, binary_path: PathBuf) -> Self {
        self.binary_path = Some(binary_path);
        self
    }

    pub fn set_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = Some(base_dir);
        self
    }

    pub fn set_endpoint(mut self, endpoint: Endpoint) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn set_disable_unsafe_disclaimer(
        mut self,
        disable_unsafe_disclaimer: bool,
    ) -> Self {
        self.disable_unsafe_disclaimer = disable_unsafe_disclaimer;
        self
    }

    pub fn build(self) -> Result<OctezClient> {
        self.validate_binary_path()?;
        let node_default_endpoint = format!("http://{}", DEFAULT_RPC_ENDPOINT);
        Ok(OctezClient {
            binary_path: self.binary_path.unwrap_or(DEFAULT_BINARY_PATH.into()),
            base_dir: match self.base_dir {
                Some(path_buf) => Directory::try_from(path_buf)?,
                None => Directory::TempDir(tempdir()?),
            },
            endpoint: self
                .endpoint
                .unwrap_or(Endpoint::try_from(Uri::from_str(&node_default_endpoint)?)?),
            disable_unsafe_disclaimer: self.disable_unsafe_disclaimer,
        })
    }

    fn validate_binary_path(&self) -> Result<()> {
        if let Some(binary_path) = &self.binary_path {
            if !binary_path.exists() {
                bail!("Binary path does not exist");
            }
            if !binary_path.is_file() {
                bail!("Binary path is not a file");
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Signature {
    ED25519,
    SECP256K1,
    P256,
    BLS,
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Signature::ED25519 => write!(f, "ed25519"),
            Signature::SECP256K1 => write!(f, "secp256k1"),
            Signature::P256 => write!(f, "p256"),
            Signature::BLS => write!(f, "bls"),
        }
    }
}

#[derive(Debug)]
pub struct Address {
    pub hash: PublicKeyHash,
    pub public_key: PublicKey,
    pub secret_key: Option<SecretKey>,
}

impl Address {
    const HASH: &'static str = "Hash";
    const PUBLIC_KEY: &'static str = "Public Key";
    const SECRET_KEY: &'static str = "Secret Key";
}

impl TryFrom<StdOut> for Address {
    type Error = anyhow::Error;
    // the output of `show address` command is expected to be in the following format:
    /*
     * Hash: tz1..
     * Public Key: edpk..
     * Secret Key: (unencrypted:)edsk..
     */
    fn try_from(stdout: StdOut) -> Result<Self> {
        if !stdout.starts_with(Self::HASH) {
            bail!("Invalid format:, {:?}", stdout);
        }
        let mut hash = None;
        let mut public_key = None;
        let mut secret_key = None;
        for line in stdout.lines() {
            if let Some((key, mut value)) = line.split_once(": ") {
                match key {
                    Self::HASH => {
                        hash = Some(PublicKeyHash::from_base58(value)?);
                    }
                    Self::PUBLIC_KEY => {
                        public_key = Some(PublicKey::from_base58(value)?);
                    }
                    Self::SECRET_KEY => {
                        if value.starts_with("unencrypted") {
                            value = value.split(':').nth(1).unwrap();
                        }
                        secret_key = Some(SecretKey::from_base58(value)?);
                    }
                    _ => bail!("Invalid key: {:?}", key),
                }
            }
        }
        Ok(Address {
            hash: hash.ok_or(anyhow!("Missing hash"))?,
            public_key: public_key.ok_or(anyhow!("Missing public key"))?,
            secret_key,
        })
    }
}

#[derive(Debug)]
pub struct OctezClient {
    binary_path: PathBuf,
    base_dir: Directory,
    endpoint: Endpoint,
    disable_unsafe_disclaimer: bool,
}

impl OctezClient {
    fn command<S: AsRef<OsStr>, I: IntoIterator<Item = S>>(
        &self,
        args: I,
    ) -> Result<Command> {
        let binary_path = self
            .binary_path
            .to_str()
            .ok_or(anyhow!("binary path must be a valid utf-8 path"))?;
        let mut command = Command::new(binary_path);
        let base_dir: String = (&self.base_dir).try_into()?;
        command.args(["--base-dir", &base_dir]);
        command.args(["--endpoint", &self.endpoint.to_string()]);
        if self.disable_unsafe_disclaimer {
            command.env("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");
        }
        command.args(args);
        Ok(command)
    }

    async fn spawn_and_wait_command<S: AsRef<OsStr>, I: IntoIterator<Item = S>>(
        &self,
        args: I,
    ) -> Result<StdOut> {
        let mut command = self.command(args)?;
        let output = command.output().await?;
        match output.status.code() {
            Some(0) => Ok(String::from_utf8(output.stdout)?),
            Some(code) => {
                let stderr = String::from_utf8(output.stderr)?;
                bail!(
                    "Command {:?} failed with exit code {}: {}",
                    command,
                    code,
                    stderr
                )
            }
            None => {
                bail!("Command terminated by a signal");
            }
        }
    }

    pub async fn config_init(&self, output_path: &Path) -> Result<()> {
        let output = output_path
            .to_str()
            .ok_or(anyhow!("config output path must be a valid utf-8 path"))?;
        self.spawn_and_wait_command(["config", "init", "--output", output])
            .await?;
        Ok(())
    }

    pub async fn gen_keys(
        &self,
        alias: &str,
        signature: Option<Signature>,
    ) -> Result<()> {
        if let Some(signature) = signature {
            let _ = self
                .spawn_and_wait_command([
                    "gen",
                    "keys",
                    alias,
                    "--sig",
                    &signature.to_string(),
                ])
                .await;
            return Ok(());
        }
        self.spawn_and_wait_command(["gen", "keys", alias]).await?;
        Ok(())
    }

    pub async fn show_address(
        &self,
        alias: &str,
        include_secret_key: bool,
    ) -> Result<Address> {
        let mut args = vec!["show", "address", alias];
        if include_secret_key {
            args.push("--show-secret");
        }
        let stdout = self.spawn_and_wait_command(args).await?;
        Address::try_from(stdout)
    }

    pub async fn import_secret_key(&self, alias: &str, secret_key: &str) -> Result<()> {
        self.spawn_and_wait_command(["import", "secret", "key", alias, secret_key])
            .await?;
        Ok(())
    }

    pub async fn activate_protocol(
        &self,
        protocol: &str,
        fitness: &str,
        key: &str,
        parameters_file: &Path,
    ) -> Result<()> {
        let args = [
            "-block",
            "genesis",
            "activate",
            "protocol",
            protocol,
            "with",
            "fitness",
            fitness,
            "and",
            "key",
            key,
            "and",
            "parameters",
            parameters_file
                .to_str()
                .ok_or(anyhow!("parameters file path must be a valid utf-8 path"))?,
        ];
        self.spawn_and_wait_command(args).await?;
        Ok(())
    }

    pub async fn add_address(
        &self,
        alias: &str,
        public_key_hash: &PublicKeyHash,
        overwrite: bool,
    ) -> Result<()> {
        let hash_string = public_key_hash.to_string();
        let mut args = vec!["add", "address", alias, &hash_string];
        if overwrite {
            args.push("-f");
        }
        self.spawn_and_wait_command(args).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    #[test]
    fn builds_default_octez_client() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        assert_eq!(
            octez_client.binary_path.to_str().unwrap(),
            DEFAULT_BINARY_PATH
        );
        assert!(matches!(octez_client.base_dir, Directory::TempDir(_)));
        assert!(!octez_client.disable_unsafe_disclaimer);
        assert_eq!(octez_client.endpoint, Endpoint::localhost(8732))
    }

    #[test]
    fn temp_dir_is_created_by_default() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        assert!(
            matches!(octez_client.base_dir, Directory::TempDir(temp_dir) if temp_dir.path().exists())
        );
    }

    #[test]
    fn temp_dir_is_removed_on_drop() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        match &octez_client.base_dir {
            Directory::TempDir(temp_dir) => {
                let temp_dir_path = temp_dir.path().to_path_buf();
                drop(octez_client);
                assert!(!temp_dir_path.exists());
            }
            _ => panic!("Expected TempDir"),
        };
    }

    #[test]
    fn sets_custom_binary_and_dir_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let binary_path = temp_file.path().to_path_buf();
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(binary_path.clone())
            .set_base_dir(dir_path.clone())
            .build()
            .unwrap();
        assert_eq!(octez_client.binary_path, binary_path);
        assert!(
            matches!(octez_client.base_dir, Directory::Path(path) if path == dir_path)
        );
    }

    #[test]
    fn validates_base_dir_exists() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let _ = std::fs::remove_file(&dir_path);
        let _ = std::fs::remove_dir_all(&dir_path);
        let octez_client = OctezClientBuilder::new()
            .set_base_dir(dir_path.clone())
            .build();
        octez_client.expect_err("Should fail when base dir does not exist ");
    }

    #[test]
    fn validates_base_dir_is_dir() {
        let temp_file = NamedTempFile::new().unwrap();
        let invalid_dir_path = temp_file.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_base_dir(invalid_dir_path.clone())
            .build();

        octez_client.expect_err("Should fail when base dir is not a directory");
    }

    #[test]
    fn validates_binary_path_exists() {
        let temp_file = NamedTempFile::new().unwrap();
        let binary_path = temp_file.path().to_path_buf();
        let _ = std::fs::remove_file(&binary_path);
        let _ = std::fs::remove_dir_all(&binary_path);
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(binary_path.clone())
            .build();
        assert!(
            octez_client.is_err_and(|e| &e.to_string() == "Binary path does not exist")
        );
    }

    #[test]
    fn validates_binary_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_binary_path = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(invalid_binary_path.clone())
            .build();
        assert!(
            octez_client.is_err_and(|e| &e.to_string() == "Binary path is not a file")
        );
    }

    #[test]
    fn commands_are_created() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_base_dir(base_dir.clone())
            .build()
            .unwrap();
        let actual = octez_client.command(["some", "command"]).unwrap();
        let actual_program = actual.as_std().get_program().to_str().unwrap();
        let actual_args = actual
            .as_std()
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<&str>>();
        let expected_program = DEFAULT_BINARY_PATH;
        let expected_args: Vec<&str> = vec![
            "--base-dir",
            base_dir.to_str().unwrap(),
            "--endpoint",
            "http://localhost:8732",
            "some",
            "command",
        ];
        assert_eq!(actual_program, expected_program);
        assert_eq!(actual_args, expected_args);
    }

    #[test]
    fn address_try_from() {
        let input_text = "Hash: tz1d5aeTJZ89RxAcuFduWRmyRUwYXfZSBVSB\nPublic Key: edpkutoN27QVVbshDg2iWTGAPDN3jywvAhzxuWm3D4Nqbn7aF8fhka\nSecret Key: edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6";
        let res = Address::try_from(input_text.to_string());
        assert!(res.is_ok_and(|addr| {
            addr.hash.to_base58() == "tz1d5aeTJZ89RxAcuFduWRmyRUwYXfZSBVSB"
                && addr.public_key.to_base58()
                    == "edpkutoN27QVVbshDg2iWTGAPDN3jywvAhzxuWm3D4Nqbn7aF8fhka"
                && addr.secret_key.unwrap().to_base58()
                    == "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
        }));
    }

    #[test]
    fn address_try_from_fails_on_invalid_input() {
        let input_text = "Wrong format";
        let res = Address::try_from(input_text.to_owned());
        assert!(res.is_err_and(|e| e.to_string().contains("Invalid format")));
    }

    #[test]
    fn address_try_from_fails_on_invalid_key() {
        let input_text = "Hash: tz1d5aeTJZ89RxAcuFduWRmyRUwYXfZSBVSB\nPublicKey: edpkutoN27QVVbshDg2iWTGAPDN3jywvAhzxuWm3D4Nqbn7aF8fhka";
        let res = Address::try_from(input_text.to_owned());
        assert!(res.is_err_and(|e| e.to_string().contains("Invalid key")));
    }

    #[test]
    fn address_try_from_fails_on_missing_public_key() {
        let input_text = "Hash: tz1d5aeTJZ89RxAcuFduWRmyRUwYXfZSBVSB";
        let res = Address::try_from(input_text.to_owned());
        assert!(res.is_err_and(|e| e.to_string().contains("Missing public key")));
    }
}
