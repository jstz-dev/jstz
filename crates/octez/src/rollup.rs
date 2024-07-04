use std::{
    fs::File,
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

const BINARY_NAME: &str = "octez-smart-rollup-node";

fn default_command() -> Command {
    Command::new(BINARY_NAME)
}

impl OctezRollupNode {
    fn command(&self) -> Command {
        let mut command = self
            .octez_setup
            .as_ref()
            .map(|setup| setup.command(BINARY_NAME, &[] as &[&str]))
            .unwrap_or_else(default_command);

        command.args(["--endpoint", &self.endpoint]);

        if let Some(path) = &self.octez_client_dir {
            command.args(["--base-dir", path.to_str().expect("Invalid path")]);
        }

        command
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
        let res = self
            .client
            .get(format!(
                "{}/global/block/head/durable/wasm_2_0_0/value?key={}",
                self.endpoint, key
            ))
            .send()
            .await?;

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
