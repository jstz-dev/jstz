use anyhow::{anyhow, bail, Context, Result};
use jstz_crypto::{
    public_key::PublicKey, public_key_hash::PublicKeyHash, secret_key::SecretKey,
};
use regex::Regex;
use std::{
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::tempdir;
use tezos_crypto_rs::hash::{BlockHash, ContractKt1Hash, OperationHash};
use tokio::process::Command;

use super::{directory::Directory, endpoint::Endpoint, node_config::OctezNodeConfig};

const DEFAULT_BINARY_PATH: &str = "octez-client";

type StdOut = String;

#[derive(Debug, Clone)]
pub struct OctezClientConfig {
    binary_path: PathBuf,
    base_dir: Arc<Directory>,
    octez_node_endpoint: Endpoint,
    disable_unsafe_disclaimer: bool,
}

impl OctezClientConfig {
    pub fn base_dir(&self) -> &Directory {
        self.base_dir.as_ref()
    }
}

pub struct OctezClientConfigBuilder {
    // if None, use the binary in $PATH
    binary_path: Option<PathBuf>,
    // if None, use temp directory
    base_dir: Option<PathBuf>,
    octez_node_endpoint: Endpoint,
    disable_unsafe_disclaimer: bool,
}

impl OctezClientConfigBuilder {
    pub fn new(octez_node_endpoint: Endpoint) -> Self {
        OctezClientConfigBuilder {
            binary_path: None,
            base_dir: None,
            octez_node_endpoint,
            disable_unsafe_disclaimer: false,
        }
    }

    pub fn with_octez_node_config(config: &OctezNodeConfig) -> Self {
        Self::new(config.rpc_endpoint.clone())
    }

    pub fn set_binary_path(mut self, binary_path: PathBuf) -> Self {
        self.binary_path = Some(binary_path);
        self
    }

    pub fn set_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = Some(base_dir);
        self
    }

    pub fn set_disable_unsafe_disclaimer(
        mut self,
        disable_unsafe_disclaimer: bool,
    ) -> Self {
        self.disable_unsafe_disclaimer = disable_unsafe_disclaimer;
        self
    }

    pub fn build(self) -> Result<OctezClientConfig> {
        self.validate_binary_path()?;
        Ok(OctezClientConfig {
            binary_path: self.binary_path.unwrap_or(DEFAULT_BINARY_PATH.into()),
            base_dir: Arc::new(match self.base_dir {
                Some(path_buf) => Directory::try_from(path_buf)?,
                None => Directory::TempDir(tempdir()?),
            }),
            octez_node_endpoint: self.octez_node_endpoint,
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

#[derive(Default)]
struct TransferOptionsBuilder {
    from: Option<String>,
    to: Option<String>,
    amount: Option<f64>,
    entrypoint: Option<String>,
    arg: Option<String>,
    burn_cap: Option<f64>,
}

impl TransferOptionsBuilder {
    pub fn new() -> Self {
        TransferOptionsBuilder {
            ..Default::default()
        }
    }

    pub fn set_from(mut self, from: String) -> Self {
        self.from = Some(from);
        self
    }

    pub fn set_to(mut self, to: String) -> Self {
        self.to = Some(to);
        self
    }

    pub fn set_entrypoint(mut self, entrypoint: String) -> Self {
        self.entrypoint = Some(entrypoint);
        self
    }

    pub fn set_arg(mut self, arg: String) -> Self {
        self.arg = Some(arg);
        self
    }

    pub fn set_amount(mut self, amount: f64) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn set_burn_cap(mut self, burn_cap: f64) -> Self {
        self.burn_cap = Some(burn_cap);
        self
    }

    pub fn build(self) -> Result<TransferOptions> {
        Ok(TransferOptions {
            from: self.from.ok_or(anyhow!("Missing from"))?,
            to: self.to.ok_or(anyhow!("Missing to"))?,
            amount: self.amount.unwrap_or_default(),
            entrypoint: self.entrypoint,
            arg: self.arg,
            burn_cap: self.burn_cap,
        })
    }
}

struct TransferOptions {
    from: String,
    to: String,
    amount: f64,
    entrypoint: Option<String>,
    arg: Option<String>,
    burn_cap: Option<f64>,
}

#[derive(Debug)]
pub struct OctezClient {
    binary_path: PathBuf,
    base_dir: Arc<Directory>,
    octez_node_endpoint: Endpoint,
    disable_unsafe_disclaimer: bool,
}

impl OctezClient {
    pub fn new(config: OctezClientConfig) -> Self {
        Self {
            binary_path: config.binary_path,
            base_dir: config.base_dir,
            octez_node_endpoint: config.octez_node_endpoint,
            disable_unsafe_disclaimer: config.disable_unsafe_disclaimer,
        }
    }
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
        let base_dir: String = self.base_dir.as_ref().try_into()?;
        command.args(["--base-dir", &base_dir]);
        command.args(["--endpoint", &self.octez_node_endpoint.to_string()]);
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

    pub fn base_dir(&self) -> &Directory {
        &self.base_dir
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

    pub async fn get_balance(&self, alias: &str) -> Result<f64> {
        let stdout = self
            .spawn_and_wait_command(["get", "balance", "for", alias])
            .await?;
        Self::extract_digits(&stdout)
    }

    /// Extract digits followed by a space and ꜩ
    /// e.g. 30000 ꜩ -> 30000
    fn extract_digits(input: &str) -> Result<f64> {
        // Define a regex pattern to capture the digits followed by a space and ꜩ
        let re = Regex::new(r"(\d+(\.\d+)?)\s*ꜩ").context("Failed to create regex")?;
        if let Some(caps) = re.captures(input) {
            caps.get(1)
                .context("Failed to capture digits")?
                .as_str()
                .parse::<f64>()
                .context("Failed to parse digits as f64")
        } else {
            Err(anyhow::anyhow!("Input string did not match the pattern"))
        }
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

    pub async fn originate_contract(
        &self,
        contract_alias: &str,
        funding_account_src: &str,
        fund_tez: f64,
        contract_path: &Path,
        init_data: Option<&str>,
        burn_cap: Option<f64>,
    ) -> Result<(ContractKt1Hash, OperationHash)> {
        let fund_str = fund_tez.to_string();
        let burn_cap_str = burn_cap.map(|v| v.to_string());
        let mut args = vec![
            "originate",
            "contract",
            contract_alias,
            "transferring",
            &fund_str,
            "from",
            funding_account_src,
            "running",
            contract_path
                .to_str()
                .ok_or(anyhow::anyhow!("failed to convert contract path to string"))?,
        ];
        if let Some(data) = init_data {
            args.append(&mut vec!["--init", data]);
        }
        if let Some(v) = &burn_cap_str {
            args.append(&mut vec!["--burn-cap", v]);
        }
        let output = self.spawn_and_wait_command(args).await?;

        let operation_hash = parse_operation_hash(&output);
        if let Err(e) = operation_hash {
            anyhow::bail!(
                "failed to parse operation hash from execution output: {:?}",
                e
            );
        }
        let contract_address = parse_contract_address(&output);
        if let Err(e) = contract_address {
            anyhow::bail!(
                "failed to parse contract address from execution output: {:?}",
                e
            );
        }
        Ok((contract_address.unwrap(), operation_hash.unwrap()))
    }

    pub async fn call_contract(
        &self,
        from: &str,
        contract: &str,
        amount: f64,
        entrypoint: &str,
        arg: &str,
        burn_cap: Option<f64>,
    ) -> Result<OperationHash> {
        let transfer_options = TransferOptionsBuilder::new()
            .set_from(from.to_string())
            .set_to(contract.to_string())
            .set_amount(amount)
            .set_entrypoint(entrypoint.to_string())
            .set_arg(arg.to_string())
            .set_burn_cap(burn_cap.unwrap_or_default())
            .build()?;
        self.transfer(transfer_options).await
    }

    async fn transfer(&self, options: TransferOptions) -> Result<OperationHash> {
        let amount: String = options.amount.to_string();
        let mut args = vec![
            "transfer",
            &amount,
            "from",
            &options.from,
            "to",
            &options.to,
        ];
        if let Some(entrypoint) = options.entrypoint.as_ref() {
            args.extend_from_slice(&["--entrypoint", entrypoint]);
        }
        if let Some(arg) = options.arg.as_ref() {
            args.extend_from_slice(&["--arg", arg]);
        }
        let burn_cap = options.burn_cap.as_ref().map(|s| s.to_string());
        if let Some(burn_cap) = burn_cap.as_ref() {
            args.extend_from_slice(&["--burn-cap", burn_cap]);
        }
        let stdout = self.spawn_and_wait_command(&args).await?;
        parse_operation_hash(&stdout)
    }

    pub async fn wait_for(
        &self,
        operation_hash: &OperationHash,
        branch: Option<&BlockHash>,
        previous_num_blocks: Option<u32>,
    ) -> Result<()> {
        let operation_str = operation_hash.to_string();
        let previous_num_blocks_str = previous_num_blocks.map(|v| v.to_string());
        let branch_str = branch.map(|v| v.to_string());
        let mut args = vec!["wait", "for", &operation_str, "to", "be", "included"];
        if let Some(v) = &previous_num_blocks_str {
            args.append(&mut vec!["--check-previous", v]);
        }
        if let Some(v) = &branch_str {
            args.append(&mut vec!["--branch", v]);
        }
        self.spawn_and_wait_command(args).await?;
        Ok(())
    }
}

fn parse_regex(pattern_str: &str, output: &str) -> Result<String> {
    let pattern = regex::Regex::new(pattern_str)?;
    Ok(pattern
        .captures(output)
        .ok_or(anyhow::anyhow!("input string does not match the pattern"))?
        .get(1)
        .ok_or(anyhow::anyhow!("cannot find the first match group"))?
        .as_str()
        .to_owned())
}

fn parse_operation_hash(output: &str) -> Result<OperationHash> {
    let raw_operation_hash =
        parse_regex("Operation hash is '(o[1-9A-HJ-NP-Za-km-z]{50})'", output)?;
    Ok(OperationHash::from_base58_check(&raw_operation_hash)?)
}

fn parse_contract_address(output: &str) -> Result<ContractKt1Hash> {
    let raw_contract_hash = parse_regex(
        "New contract (KT1[1-9A-HJ-NP-Za-km-z]{33}) originated.",
        output,
    )?;
    Ok(ContractKt1Hash::from_base58_check(&raw_contract_hash)?)
}

#[cfg(test)]
mod test {
    use crate::r#async::node_config::OctezNodeConfigBuilder;

    use super::*;
    use tempfile::{NamedTempFile, TempDir};
    #[test]
    fn builds_octez_client() {
        let endpoint = Endpoint::default();
        let octez_client = OctezClient::new(
            OctezClientConfigBuilder::new(endpoint.clone())
                .build()
                .unwrap(),
        );
        assert_eq!(
            octez_client.binary_path.to_str().unwrap(),
            DEFAULT_BINARY_PATH
        );
        assert!(matches!(
            octez_client.base_dir.as_ref(),
            Directory::TempDir(_)
        ));
        assert!(!octez_client.disable_unsafe_disclaimer);
        assert_eq!(octez_client.octez_node_endpoint, endpoint)
    }
    #[test]
    fn builds_octez_client_with_node_config() {
        let node_config = OctezNodeConfigBuilder::new().build().unwrap();
        let octez_client = OctezClient::new(
            OctezClientConfigBuilder::with_octez_node_config(&node_config)
                .build()
                .unwrap(),
        );
        assert_eq!(
            octez_client.binary_path.to_str().unwrap(),
            DEFAULT_BINARY_PATH
        );
        assert!(matches!(
            octez_client.base_dir.as_ref(),
            Directory::TempDir(_)
        ));
        assert!(!octez_client.disable_unsafe_disclaimer);
        assert_eq!(octez_client.octez_node_endpoint, node_config.rpc_endpoint)
    }

    #[test]
    fn temp_dir_is_created_by_default() {
        let endpoint = Endpoint::default();
        let octez_client =
            OctezClient::new(OctezClientConfigBuilder::new(endpoint).build().unwrap());
        assert!(
            matches!(octez_client.base_dir.as_ref(), Directory::TempDir(temp_dir) if temp_dir.path().exists())
        );
    }

    #[test]
    fn temp_dir_is_removed_on_drop() {
        let endpoint = Endpoint::default();
        let octez_client =
            OctezClient::new(OctezClientConfigBuilder::new(endpoint).build().unwrap());
        match &octez_client.base_dir.as_ref() {
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
        let endpoint = Endpoint::default();
        let temp_file = NamedTempFile::new().unwrap();
        let binary_path = temp_file.path().to_path_buf();
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let octez_client = OctezClient::new(
            OctezClientConfigBuilder::new(endpoint)
                .set_binary_path(binary_path.clone())
                .set_base_dir(dir_path.clone())
                .build()
                .unwrap(),
        );
        assert_eq!(octez_client.binary_path, binary_path);
        assert!(
            matches!(octez_client.base_dir.as_ref(), Directory::Path(path) if path == &dir_path)
        );
    }

    #[test]
    fn validates_base_dir_exists() {
        let temp_dir = TempDir::new().unwrap();
        let endpoint = Endpoint::default();
        let dir_path = temp_dir.path().to_path_buf();
        let _ = std::fs::remove_file(&dir_path);
        let _ = std::fs::remove_dir_all(&dir_path);
        let octez_client_config = OctezClientConfigBuilder::new(endpoint)
            .set_base_dir(dir_path.clone())
            .build();
        octez_client_config.expect_err("Should fail when base dir does not exist ");
    }

    #[test]
    fn validates_base_dir_is_dir() {
        let endpoint = Endpoint::default();
        let temp_file = NamedTempFile::new().unwrap();
        let invalid_dir_path = temp_file.path().to_path_buf();
        let octez_client_config = OctezClientConfigBuilder::new(endpoint)
            .set_base_dir(invalid_dir_path.clone())
            .build();

        octez_client_config.expect_err("Should fail when base dir is not a directory");
    }

    #[test]
    fn validates_binary_path_exists() {
        let temp_file = NamedTempFile::new().unwrap();
        let endpoint = Endpoint::default();
        let binary_path = temp_file.path().to_path_buf();
        let _ = std::fs::remove_file(&binary_path);
        let _ = std::fs::remove_dir_all(&binary_path);
        let octez_client_config = OctezClientConfigBuilder::new(endpoint)
            .set_binary_path(binary_path.clone())
            .build();
        assert!(octez_client_config
            .is_err_and(|e| &e.to_string() == "Binary path does not exist"));
    }

    #[test]
    fn validates_binary_path_is_file() {
        let endpoint = Endpoint::default();
        let temp_dir = TempDir::new().unwrap();
        let invalid_binary_path = temp_dir.path().to_path_buf();
        let octez_client_config = OctezClientConfigBuilder::new(endpoint)
            .set_binary_path(invalid_binary_path.clone())
            .build();
        assert!(octez_client_config
            .is_err_and(|e| &e.to_string() == "Binary path is not a file"));
    }

    #[test]
    fn commands_are_created() {
        let endpoint = Endpoint::default();
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();
        let octez_client = OctezClient::new(
            OctezClientConfigBuilder::new(endpoint)
                .set_base_dir(base_dir.clone())
                .build()
                .unwrap(),
        );
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
            "http://localhost:80",
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

    #[test]
    fn extract_integer() {
        let input = "30000 ꜩ";
        let res = OctezClient::extract_digits(input);
        assert!(res.is_ok_and(|balance| balance == 30000f64));
        let input = "random text";
        let res = OctezClient::extract_digits(input);
        assert!(res.is_err_and(|e| e
            .to_string()
            .contains("Input string did not match the pattern")));
    }
    #[test]
    fn extract_float() {
        let input = "15.23453 ꜩ";
        let res = OctezClient::extract_digits(input);
        assert!(res.is_ok_and(|balance| balance == 15.23453));
    }

    #[test]
    fn test_parse_contract_address() {
        let contract_hash = "KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ";
        assert_eq!(
            parse_contract_address(&format!("New contract {contract_hash} originated."))
                .unwrap(),
            ContractKt1Hash::from_base58_check(contract_hash).unwrap()
        );
    }

    #[test]
    fn parse_contract_address_pattern_mismatch() {
        let contract_hash = "foobar";
        assert_eq!(
            parse_contract_address(&format!("New contract {contract_hash} originated."))
                .unwrap_err()
                .to_string(),
            "input string does not match the pattern"
        );
    }

    #[test]
    fn parse_contract_address_bad_hash() {
        let contract_hash = format!("KT1{}", "1".repeat(33));
        assert_eq!(
            parse_contract_address(&format!("New contract {contract_hash} originated."))
                .unwrap_err()
                .to_string(),
            "invalid checksum"
        );
    }

    #[test]
    fn test_parse_operation_hash() {
        let operation_hash = "op15fmUF1cypvbX6H5Uu8i1Fwtf2fX3vFk8yxGQLoZDCfvqht2i";
        assert_eq!(
            parse_operation_hash(&format!("Operation hash is '{operation_hash}'"))
                .unwrap(),
            OperationHash::from_base58_check(operation_hash).unwrap()
        );
    }

    #[test]
    fn parse_operation_hash_pattern_mismatch() {
        assert_eq!(
            parse_operation_hash("Operation hash is 'foobar'")
                .unwrap_err()
                .to_string(),
            "input string does not match the pattern"
        );
    }

    #[test]
    fn parse_operation_hash_bad_hash() {
        assert_eq!(
            parse_operation_hash(&format!("Operation hash is 'o{}'", "1".repeat(50)))
                .unwrap_err()
                .to_string(),
            "invalid checksum"
        );
    }

    #[test]
    fn transfer_options() {
        let options = TransferOptionsBuilder::new()
            .set_from("tz1".to_string())
            .set_to("tz2".to_string())
            .set_entrypoint("entrypoint".to_string())
            .set_arg("arg".to_string())
            .set_amount(100.3f64)
            .set_burn_cap(999f64)
            .build()
            .unwrap();
        assert_eq!(&options.from, "tz1");
        assert_eq!(&options.to, "tz2");
        assert_eq!(options.entrypoint.unwrap(), "entrypoint");
        assert_eq!(options.arg.unwrap(), "arg");
        assert_eq!(options.amount, 100.3f64);
        assert_eq!(options.burn_cap.unwrap(), 999f64);
    }

    #[test]
    fn transfer_options_default() {
        let options = TransferOptionsBuilder::new()
            .set_from("tz1".to_string())
            .set_to("tz2".to_string())
            .build()
            .unwrap();
        assert_eq!(options.from, "tz1");
        assert_eq!(options.to, "tz2");
        assert_eq!(options.entrypoint, None);
        assert_eq!(options.arg, None);
        assert_eq!(options.amount, 0f64);
        assert_eq!(options.burn_cap, None);
    }

    #[test]
    fn missing_from_transfer_options_throws() {
        let options = TransferOptionsBuilder::new().build();
        assert!(options.is_err_and(|e| e.to_string().contains("Missing from")));
    }

    #[test]
    fn missing_to_transfer_options_throws() {
        let options = TransferOptionsBuilder::new()
            .set_from("tz1".to_string())
            .build();
        assert!(options.is_err_and(|e| e.to_string().contains("Missing to")));
    }
}
