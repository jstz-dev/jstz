use super::{directory::Directory, endpoint::Endpoint, octez_node::DEFAULT_RPC_ENDPOINT};
use anyhow::{anyhow, bail, Result};
use http::Uri;
use std::path::Path;
use std::{ffi::OsStr, fmt, path::PathBuf, str::FromStr};
use tempfile::tempdir;
use tokio::process::Command;

const DEFAULT_BINARY_PATH: &str = "octez-client";

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
    ) -> Result<()> {
        let mut command = self.command(args)?;
        let status = command.spawn()?.wait().await?;
        match status.code() {
            Some(0) => Ok(()),
            Some(code) => bail!("Command {:?} failed with exit code {}", command, code),
            None => bail!("Command terminated by a signal"),
        }
    }

    pub async fn config_init(&self, output_path: &Path) -> Result<()> {
        let output = output_path
            .to_str()
            .ok_or(anyhow!("config output path must be a valid utf-8 path"))?;
        self.spawn_and_wait_command(["config", "init", "--output", output])
            .await
    }

    pub async fn gen_keys(
        &self,
        alias: &str,
        signature: Option<Signature>,
    ) -> Result<()> {
        if let Some(signature) = signature {
            return self
                .spawn_and_wait_command([
                    "gen",
                    "keys",
                    alias,
                    "--sig",
                    &signature.to_string(),
                ])
                .await;
        }
        self.spawn_and_wait_command(["gen", "keys", alias]).await
    }

    pub async fn import_secret_key(&self, alias: &str, secret_key: &str) -> Result<()> {
        self.spawn_and_wait_command(["import", "secret", "key", alias, secret_key])
            .await
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
}
