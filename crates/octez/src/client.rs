use std::{
    fs::File,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Result};
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{path_or_default, run_command, run_command_with_output};

#[derive(Debug, Serialize, Deserialize)]
pub struct OctezClient {
    /// Path to the octez-client binary
    /// If None, the binary will inside PATH will be used
    pub octez_client_bin: Option<PathBuf>,
    /// If None, the default directory will be used (~/.tezos-client/)
    pub octez_client_dir: Option<PathBuf>,
    /// RPC endpoint for the octez-node
    pub endpoint: String,
    /// Disable the disclaimer prompt
    pub disable_disclaimer: bool,
}

fn regex_extract(re: &str, output: &str) -> Result<String> {
    let regex = Regex::new(re)?;

    let capture = regex
        .captures(output)
        .ok_or_else(|| anyhow!("Unexpected output from octez-client"))?
        .get(1)
        .ok_or_else(|| anyhow!("Unexpected output from octez-client"))?
        .as_str()
        .to_string();

    Ok(capture)
}

#[derive(Debug, Clone)]
pub struct AliasInfo {
    pub address: String,
    pub public_key: String,
    pub secret_key: String,
}

impl OctezClient {
    pub fn new(
        octez_client_bin: Option<PathBuf>,
        octez_client_dir: Option<PathBuf>,
        endpoint: String,
        disable_disclaimer: bool,
    ) -> Self {
        Self {
            octez_client_bin,
            octez_client_dir,
            endpoint,
            disable_disclaimer,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(path_or_default(
            self.octez_client_bin.as_ref(),
            "octez-client",
        ));

        if let Some(path) = &self.octez_client_dir {
            command.args(["--base-dir", path.to_str().expect("Invalid path")]);
        }

        command.args(["--endpoint", &self.endpoint]);

        if self.disable_disclaimer {
            command.env("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");
        }

        command
    }

    /// Generate a new key with the given `alias`
    pub fn gen_keys(&self, alias: &str) -> Result<()> {
        run_command(self.command().args(["gen", "keys", alias, "--force"]))
    }

    /// Originate a Michelson smart contract with the given `name` using account `source`
    /// and `script` as code and `storage` as initial storage.
    pub fn originate_contract(
        &self,
        name: &str,
        source: &str,
        script: &str,
        storage: &str,
    ) -> Result<String> {
        let output = run_command_with_output(self.command().args([
            "originate",
            "contract",
            name,
            "transferring",
            "0",
            "from",
            source,
            "running",
            script,
            "--init",
            storage,
            "--burn-cap",
            "999",
            "--force",
        ]))?;

        regex_extract(r"New contract (KT1[^\s]+) originated\.", &output)
    }

    /// Call contract `to` using account `from` with the given `entrypoint` and `parameter`.
    pub fn call_contract(
        &self,
        from: &str,
        to: &str,
        entrypoint: &str,
        parameter: &str,
        amount: &Decimal,
    ) -> Result<()> {
        let amount = amount.to_string();
        run_command(self.command().args([
            "transfer",
            &amount,
            "from",
            from,
            "to",
            to,
            "--entrypoint",
            entrypoint,
            "--arg",
            parameter,
            "--burn-cap",
            "999",
        ]))
    }

    /// Send an external message to the smart rollup `to` using account `from`.
    pub fn send_rollup_external_message<T: AsRef<[u8]>>(
        &self,
        source: &str,
        message: T,
    ) -> Result<()> {
        run_command(self.command().args([
            "send",
            "smart",
            "rollup",
            "message",
            &format!("hex:[ \"{}\" ]", hex::encode(message)),
            "from",
            source,
        ]))
    }

    /// Run arbitrary RPC command
    pub fn rpc(&self, options: &[&str]) -> Result<String> {
        run_command_with_output(self.command().arg("rpc").args(options))
    }

    /// Blocks until the node is bootstrapped
    pub fn wait_for_node_to_bootstrap(&self) -> Result<()> {
        run_command(self.command().arg("bootstrapped"))
    }

    /// Forge and inject block (bake) using the delegate's rights
    pub fn bake(&self, log_file: &File, options: &[&str]) -> Result<String> {
        run_command_with_output(
            self.command()
                .arg("bake")
                .stdout(Stdio::from(log_file.try_clone()?))
                .stderr(Stdio::from(log_file.try_clone()?))
                .args(options),
        )
    }

    /// Import a secret key `sk` with the given `name`
    pub fn import_secret_key(&self, name: &str, sk: &str) -> Result<()> {
        run_command(
            self.command()
                .args(["import", "secret", "key", name, sk, "--force"]),
        )
    }

    pub fn alias_info(&self, alias: &str) -> Result<AliasInfo> {
        let output = run_command_with_output(self.command().args([
            "show",
            "address",
            alias,
            "--show-secret",
        ]))?;

        let address = regex_extract(r"Hash: ([^\s]+)", &output)?;
        let public_key = regex_extract(r"Public Key: ([^\s]+)", &output)?;
        let secret_key = regex_extract(r"Secret Key: ([^\s]+)", &output)?;

        Ok(AliasInfo {
            address,
            public_key,
            secret_key,
        })
    }

    pub fn resolve_contract(&self, alias: &str) -> Result<String> {
        let output = run_command_with_output(
            self.command().args(["show", "known", "contract", alias]),
        )?;

        let address = regex_extract(r"(KT1[^\s]+)", &output)?;
        Ok(address)
    }

    pub fn resolve_jstz_addres(&self, alias: &str) -> Result<String> {
        let output = run_command_with_output(
            self.command()
                .args(["show", "known", "smart", "rollup", alias]),
        )?;

        let address = regex_extract(r"(sr1[^\s]+)", &output)?;
        Ok(address)
    }

    /// Retrieve the info of an account with the given `alias`
    pub fn balance(&self, account: &str) -> Result<u64> {
        let output = run_command_with_output(
            self.command().args(["get", "balance", "for", account]),
        )?;

        let output = regex_extract(r"(\d+|\d*\.\d+) ꜩ", &output)?;

        Ok(output.parse::<u64>()?)
    }

    /// Activate a protocol with the given `protocol`, `fitness`, `key` and `parameters_file`
    pub fn activate_protocol(
        &self,
        protocol: &str,
        fitness: &str,
        key: &str,
        parameters_file: &str,
    ) -> Result<()> {
        run_command(self.command().args([
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
            parameters_file,
        ]))
    }

    fn wait_for_operation(&self, operation: &str) -> Result<()> {
        run_command(self.command().args([
            "wait",
            "for",
            operation,
            "to",
            "be",
            "included",
            "--confirmations",
            "2",
        ]))
    }

    /// Originate a smart rollup
    pub fn originate_rollup(
        &self,
        account: &str,
        name: &str,
        kind: &str,
        r#type: &str,
        kernel: &str,
    ) -> Result<String> {
        let output = run_command_with_output(self.command().args([
            "originate",
            "smart",
            "rollup",
            name,
            "from",
            account,
            "of",
            "kind",
            kind,
            "of",
            "type",
            r#type,
            "with",
            "kernel",
            kernel,
            "--burn-cap",
            "999",
            "--force",
            "--whitelist",
            format!("[\"{account}\"]").as_str(),
        ]))?;

        let operation = regex_extract(r"Operation hash is '(o[^\s]+)'", &output)?;

        self.wait_for_operation(&operation)?;

        regex_extract(r"Address: (sr1[^\s]+)", &output)
    }
}
