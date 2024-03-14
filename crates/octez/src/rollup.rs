use std::{
    fs::File,
    //intrinsics::prefetch_read_instruction,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::OctezSetup;

#[derive(Debug, Serialize, Deserialize)]
pub struct OctezRollupNode {
    /// Setup for Octez smart rollup node (process path or Docker container)
    pub octez_setup: Option<OctezSetup>,
    /// Path to the octez-smart-rollup-node directory
    pub octez_rollup_node_dir: PathBuf,
    /// If None, the default directory will be used (~/.tezos-client/)
    pub octez_client_dir: Option<PathBuf>,
    /// RPC endpoint for the octez-node
    pub endpoint: String,
}

impl OctezRollupNode {
    fn command(&self) -> Command {
        match &self.octez_setup {
            Some(OctezSetup::Process(path)) => {
                let bin_path = path.join("octez-smart-rollup-node");
                let mut command = Command::new(bin_path);
                self.configure_command(&mut command);
                command
            }
            Some(OctezSetup::Docker(container_name)) => {
                let mut command = Command::new("docker");
                command.args([
                    "run",
                    "--network=host",
                    "--entrypoint=/usr/local/bin/octez-smart-rollup-node",
                    "-v",
                    "/var:/var",
                    "-v",
                    "/tmp:/tmp",
                    container_name,
                ]);
                self.configure_command(&mut command);
                command
            }
            None => {
                let mut command = Command::new("octez-smart-rollup-node");
                self.configure_command(&mut command);
                command
            }
        }
    }

    /// Configures the command with common arguments.
    fn configure_command(&self, command: &mut Command) {
        command.args(["--endpoint", &self.endpoint]);

        if let Some(path) = &self.octez_client_dir {
            command.args(["--base-dir", path.to_str().expect("Invalid path")]);
        }
    }

    /// Run a smart rollup operator
    pub fn run(
        &self,
        addr: &str,
        port: u16,
        log_file: &File,
        rollup: &str,
        operator: &str,
        options: &[&str],
    ) -> Result<Child> {
        let mut binding = self.command();
        let c = binding
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?))
            .args([
                "run",
                "operator",
                "for",
                rollup,
                "with",
                "operators",
                operator,
                "--data-dir",
                self.octez_rollup_node_dir.to_str().expect("Invalid path"),
                "--rpc-addr",
                addr,
                "--rpc-port",
                &port.to_string(),
            ])
            .args(options);
        println!("{:?}", c);
        Ok(self
            .command()
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file.try_clone()?))
            .args([
                "run",
                "operator",
                "for",
                rollup,
                "with",
                "operators",
                operator,
                "--data-dir",
                self.octez_rollup_node_dir.to_str().expect("Invalid path"),
                "--rpc-addr",
                addr,
                "--rpc-port",
                &port.to_string(),
                "--acl-override",
                "allow-all",
            ])
            .args(options)
            .spawn()?)
    }
}

#[derive(Debug)]
pub struct OctezRollupClient {
    endpoint: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct ValueError {
    pub kind: String,
    pub id: String,
    pub block: Option<String>,
    pub msg: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ValueResponse {
    Value(String),
    Errors(Vec<ValueError>),
}

#[derive(Debug, Deserialize)]
struct SubkeysResponse(Vec<String>);

impl OctezRollupClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    pub async fn batcher_injection<S, I>(&self, external_messages: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<[u8]>,
    {
        let res = self
            .client
            .post(format!("{}/local/batcher/injection", self.endpoint))
            .json(
                &external_messages
                    .into_iter()
                    .map(hex::encode)
                    .collect::<Vec<String>>(),
            )
            .send()
            .await?;

        if res.status() == 200 {
            // TODO: Should we ignore the response?
            Ok(())
        } else {
            Err(anyhow!("Unhandled response status: {}", res.status()))
        }
    }

    pub async fn get_value(&self, key: &str) -> Result<Option<Vec<u8>>> {
        println!("Getting value for key: {}", key);
        let res = self
            .client
            .get(format!(
                "{}/global/block/head/durable/wasm_2_0_0/value?key={}",
                self.endpoint, key
            ))
            .send()
            .await;

        println!("Response: {:?}", res);

        let res = res?;

        if res.status() == 200 || res.status() == 500 {
            let content: Option<ValueResponse> = res.json().await?;
            match content {
                Some(ValueResponse::Value(value)) => {
                    let payload = hex::decode(value)?;
                    Ok(Some(payload))
                }
                Some(ValueResponse::Errors(errors)) => Err(anyhow!(
                    "Failed to get value of key-value pair: {}. Errors: {:?}",
                    key,
                    errors
                )),
                None => Ok(None),
            }
        } else {
            Err(anyhow!("Unhandled response status: {}", res.status()))
        }
    }

    pub async fn get_subkeys(&self, key: &str) -> Result<Option<Vec<String>>> {
        let res = self
            .client
            .get(format!(
                "{}/global/block/head/durable/wasm_2_0_0/subkeys?key={}",
                self.endpoint, key
            ))
            .send()
            .await?;

        if res.status() == 200 || res.status() == 500 {
            let content =
                serde_json::from_str::<SubkeysResponse>(res.text().await?.as_str());

            match content {
                Ok(SubkeysResponse(subkeys)) => Ok(Some(subkeys)),
                Err(error) => Err(anyhow!(
                    "Failed to get subkeys for {}. Error: {:?}",
                    key,
                    error
                )),
            }
        } else {
            Err(anyhow!("Unhandled response status: {}", res.status()))
        }
    }
}
