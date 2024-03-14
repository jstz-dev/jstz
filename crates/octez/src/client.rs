use std::{
    fs::File,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{run_command, run_command_with_output, OctezSetup};

#[derive(Debug, Serialize, Deserialize)]
pub struct OctezClient {
    /// /// Setup for Octez client (process path or Docker container)
    pub octez_setup: Option<OctezSetup>,
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
        octez_setup: Option<OctezSetup>,
        octez_client_dir: Option<PathBuf>,
        endpoint: String,
        disable_disclaimer: bool,
    ) -> Self {
        Self {
            octez_setup,
            octez_client_dir,
            endpoint,
            disable_disclaimer,
        }
    }

    fn command(&self, mounts: &[String]) -> Command {
        match &self.octez_setup {
            Some(OctezSetup::Process(path)) => {
                let bin_path = path.join("octez-client");
                let mut command = Command::new(bin_path);
                self.configure_command(&mut command);
                command
            }
            Some(OctezSetup::Docker(container_name)) => {
                let mut cmd = Command::new("docker");

                let mut args = vec![
                    "run".to_string(),
                    "--network=host".to_string(),
                    "--entrypoint=/usr/local/bin/octez-client".to_string(),
                    "-v".to_string(),
                    "/var:/var".to_string(),
                    "-v".to_string(),
                    "/tmp:/tmp".to_string(),
                ];

                // Iterate over the host paths to mount, using fixed container paths
                for path in mounts {
                    args.push("-v".to_string());
                    args.push(format!("{}:{}", path, path));
                }

                args.push(container_name.to_string());

                cmd.args(args);

                self.configure_command(&mut cmd);
                cmd
            }
            None => {
                let mut command = Command::new("octez-client");
                self.configure_command(&mut command);
                command
            }
        }
    }

    /// Configures the command with common arguments.
    fn configure_command(&self, command: &mut Command) {
        if let Some(path) = &self.octez_client_dir {
            command.args(["--base-dir", path.to_str().expect("Invalid path")]);
        }

        command.args(["--endpoint", &self.endpoint]);

        if self.disable_disclaimer {
            command.env("TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER", "Y");
        }
    }

    /// Generate a new key with the given `alias`
    pub fn gen_keys(&self, alias: &str) -> Result<()> {
        run_command(self.command(&[]).args(["gen", "keys", alias, "--force"]))
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
        let output = run_command_with_output(self.command(&[]).args([
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

        println!("{}", output);

        regex_extract(r"New contract (KT1[^\s]+) originated\.", &output)
    }

    /// Call contract `to` using account `from` with the given `entrypoint` and `parameter`.
    pub fn call_contract(
        &self,
        from: &str,
        to: &str,
        entrypoint: &str,
        parameter: &str,
    ) -> Result<()> {
        run_command(self.command(&[]).args([
            "transfer",
            "0",
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
        run_command(self.command(&[]).args([
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
    pub fn rpc(
        &self,
        _sandbox_host_address: &str,
        _sandbox_port: u16,
        options: &[&str],
    ) -> Result<String> {
        println!("rpc");
        run_command_with_output(self.command(&[]).args(["rpc"]).args(options))
    }

    /// Blocks until the node is bootstrapped
    pub fn wait_for_node_to_bootstrap(&self) -> Result<()> {
        run_command(self.command(&[]).arg("bootstrapped"))
    }

    /// Forge and inject block (bake) using the delegate's rights
    pub fn bake(&self, log_file: &File, options: &[&str]) -> Result<String> {
        run_command_with_output(
            self.command(&[])
                .arg("bake")
                .stdout(Stdio::from(log_file.try_clone()?))
                .stderr(Stdio::from(log_file.try_clone()?))
                .args(options),
        )
    }

    /// Import a secret key `sk` with the given `name`
    pub fn import_secret_key(&self, name: &str, sk: &str) -> Result<()> {
        run_command(
            self.command(&[])
                .args(["import", "secret", "key", name, sk, "--force"]),
        )
    }

    pub fn alias_info(&self, alias: &str) -> Result<AliasInfo> {
        let output = run_command_with_output(self.command(&[]).args([
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

    /// Retrieve the info of an account with the given `alias`
    pub fn balance(&self, account: &str) -> Result<u64> {
        let output = run_command_with_output(
            self.command(&[]).args(["get", "balance", "for", account]),
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
        sandbox_params_path: &str,
    ) -> Result<()> {
        run_command(self.command(&[sandbox_params_path.to_string()]).args([
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
        run_command(self.command(&[]).args([
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
        let output = run_command_with_output(self.command(&[]).args([
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
        ]))?;

        let operation = regex_extract(r"Operation hash is '(o[^\s]+)'", &output)?;

        self.wait_for_operation(&operation)?;

        regex_extract(r"Address: (sr1[^\s]+)", &output)
    }
}
