mod bootstrap;

pub use bootstrap::BootstrapAccount;
use bootstrap::BootstrapAccounts;
use rust_embed::Embed;
use serde_json::Value;
use std::fmt::Display;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub trait ReadWritable: Read + Write {}

impl ReadWritable for tempfile::NamedTempFile {}

#[derive(PartialEq, Eq, Debug)]
pub enum ProtocolConstants {
    Sandbox,
}

impl Default for ProtocolConstants {
    fn default() -> Self {
        Self::Sandbox
    }
}

impl Display for ProtocolConstants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Sandbox => "sandbox",
        })
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Protocol {
    Alpha,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Alpha
    }
}

impl Protocol {
    fn hash(&self) -> &'static str {
        match self {
            Protocol::Alpha => "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
        }
    }

    fn parameter_file(&self, constants: &ProtocolConstants) -> PathBuf {
        Path::new(&constants.to_string()).join(self.hash())
    }
}

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/protocol_parameters/"]
pub struct ProtocolParameterFile;

#[derive(Default)]
pub struct ProtocolParameterBuilder {
    /// Target protocol version.
    protocol: Option<Protocol>,
    /// Protocol constants.
    constants: Option<ProtocolConstants>,
    /// Bootstrap accounts.
    bootstrap_accounts: BootstrapAccounts,
    /// Path to an existing parameter file whose content will be used as the base
    /// parameter set. Optional. If `source_path` is not given, a predefined parameter
    /// file will be used instead depending on `protocol` and `constants`.
    source_path: Option<PathBuf>,
}

impl ProtocolParameterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_protocol(&mut self, protocol: Protocol) -> &mut Self {
        self.protocol.replace(protocol);
        self
    }

    pub fn set_constants(&mut self, constants: ProtocolConstants) -> &mut Self {
        self.constants.replace(constants);
        self
    }

    pub fn set_bootstrap_accounts(
        &mut self,
        accounts: Vec<BootstrapAccount>,
    ) -> &mut Self {
        for account in accounts {
            self.bootstrap_accounts.add_account(account);
        }
        self
    }

    pub fn set_source_path(&mut self, path: &str) -> &mut Self {
        self.source_path = Some(PathBuf::from(path));
        self
    }

    pub fn build(&mut self) -> anyhow::Result<impl ReadWritable> {
        let protocol = self.protocol.take();
        let constants = self.constants.take();
        let source_path = self.source_path.take();
        let mut raw_json = self.load_parameter_json(source_path, protocol, constants)?;
        let json = raw_json.as_object_mut().ok_or(anyhow::anyhow!(
            "Failed to convert loaded json file into a json object"
        ))?;

        self.inject_bootstrap_contracts(json)?;

        let mut output_file = tempfile::NamedTempFile::new().unwrap();
        serde_json::to_writer(output_file.as_file(), &json)?;
        output_file.flush()?;
        output_file.rewind()?;
        Ok(output_file)
    }

    fn load_parameter_json(
        &self,
        source_path: Option<PathBuf>,
        protocol: Option<Protocol>,
        constants: Option<ProtocolConstants>,
    ) -> anyhow::Result<Value> {
        let raw_json: Value = match source_path {
            Some(path) => {
                let mut buffer = String::new();
                match std::fs::File::open(&path) {
                    Ok(mut f) => {
                        f.read_to_string(&mut buffer)?;
                        serde_json::from_slice(buffer.as_bytes())?
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Failed to open parameter file at {:?}: {}",
                            path,
                            e.to_string()
                        ))
                    }
                }
            }
            None => {
                let file_path = protocol
                    .unwrap_or_default()
                    .parameter_file(&constants.unwrap_or_default());
                let file_path_str = file_path.to_str().ok_or(anyhow::anyhow!(
                    "Failed to convert parameter file path to string"
                ))?;
                let f =
                    ProtocolParameterFile::get(file_path_str).ok_or(anyhow::anyhow!(
                        "Failed to load parameter file at '{}'",
                        file_path_str
                    ))?;
                serde_json::from_slice(&f.data)?
            }
        };
        Ok(raw_json)
    }

    fn inject_bootstrap_contracts(
        &mut self,
        json: &mut serde_json::Map<String, Value>,
    ) -> anyhow::Result<()> {
        let mut accounts = BootstrapAccounts::default();
        if let Some(value) = json.get("bootstrap_accounts") {
            if let Some(existing_accounts) = value.as_array() {
                for account in existing_accounts {
                    accounts.add_account(BootstrapAccount::try_from(account)?);
                }
            }
        }
        for account in self.bootstrap_accounts.get_accounts() {
            accounts.add_account(account.clone());
        }
        json.insert("bootstrap_accounts".to_owned(), Value::from(&accounts));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, Write};

    use tempfile::NamedTempFile;

    use super::{
        BootstrapAccount, Protocol, ProtocolConstants, ProtocolParameterBuilder,
    };

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";

    fn create_dummy_source_file(
        bootstrap_accounts: Option<Vec<BootstrapAccount>>,
    ) -> NamedTempFile {
        let mut source_file = tempfile::NamedTempFile::new().unwrap();
        let mut json = serde_json::json!({"foo":"bar"});
        if let Some(accounts) = bootstrap_accounts {
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "bootstrap_accounts".to_owned(),
                serde_json::Value::Array(
                    accounts
                        .iter()
                        .map(|v| {
                            serde_json::Value::Array(vec![
                                serde_json::Value::String(v.get_public_key().to_string()),
                                serde_json::Value::String(v.get_amount().to_string()),
                            ])
                        })
                        .collect::<Vec<serde_json::Value>>(),
                ),
            );
        }
        serde_json::to_writer(source_file.as_file(), &json).unwrap();
        source_file.flush().unwrap();
        source_file.rewind().unwrap();
        source_file
    }

    #[test]
    fn parameter_builder() {
        let mut builder = ProtocolParameterBuilder::new();
        builder
            .set_constants(ProtocolConstants::Sandbox)
            .set_protocol(Protocol::Alpha)
            .set_source_path("/test/path")
            .set_bootstrap_accounts(
                [BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()].to_vec(),
            );
        assert_eq!(builder.constants.unwrap(), ProtocolConstants::Sandbox);
        assert_eq!(builder.source_path.unwrap().to_str().unwrap(), "/test/path");
        assert_eq!(builder.protocol.unwrap().hash(), Protocol::Alpha.hash());
        assert_eq!(builder.bootstrap_accounts.get_accounts().len(), 1);
        assert_eq!(
            builder.bootstrap_accounts.get_accounts().last().unwrap(),
            &BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()
        );
    }

    #[test]
    fn parameter_builder_default() {
        let mut builder = ProtocolParameterBuilder::new();
        // builder should be able to find the template file with default values
        // and write an output file, so we check if the result is ok here
        assert!(builder.build().is_ok());
    }

    #[test]
    fn build_parameters_from_given_file() {
        let mut builder = ProtocolParameterBuilder::new();
        let source_file = create_dummy_source_file(None);
        builder.set_source_path(source_file.path().to_str().unwrap());

        let output_file = builder.build().unwrap();
        let json: serde_json::Value = serde_json::from_reader(output_file).unwrap();

        // this output file should have the values as the source file above
        assert_eq!(json.get("foo").unwrap().as_str().unwrap(), "bar");
    }

    #[test]
    fn set_bootstrap_accounts() {
        let mut builder = ProtocolParameterBuilder::new();
        builder.set_bootstrap_accounts(
            [BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()].to_vec(),
        );
        let output_file = builder.build().unwrap();
        let json: serde_json::Value = serde_json::from_reader(output_file).unwrap();

        let accounts = json.get("bootstrap_accounts").unwrap().as_array().unwrap();
        assert_eq!(accounts.len(), 1);
        let account = accounts.last().unwrap().as_array().unwrap();
        assert_eq!(
            account.get(0).unwrap().as_str().unwrap(),
            ACCOUNT_PUBLIC_KEY
        );
        assert_eq!(account.get(1).unwrap().as_str().unwrap(), "1000");
    }

    #[test]
    fn merge_existing_bootstrap_accounts() {
        let mut builder = ProtocolParameterBuilder::new();
        let source_file = create_dummy_source_file(Some(vec![BootstrapAccount::new(
            "edpkvLEsnuq1TnYX9uc4Mcig9AiP7m3VtHpNGViDBivbvYwzzhzZRx",
            1,
        )
        .unwrap()]));
        builder
            .set_source_path(&source_file.path().to_str().unwrap())
            .set_bootstrap_accounts(
                [BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()].to_vec(),
            );
        let output_file = builder.build().unwrap();
        let json: serde_json::Value = serde_json::from_reader(output_file).unwrap();

        let mut accounts = json
            .get("bootstrap_accounts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| BootstrapAccount::try_from(v).unwrap())
            .collect::<Vec<BootstrapAccount>>();
        assert_eq!(accounts.len(), 2);
        accounts.sort_by_key(|v| v.get_amount());

        // accounts sorted by tez in ascending order, so the first one is the existing account
        // with 1 mutez only
        let existing_account = accounts.first().unwrap();
        assert_eq!(existing_account.get_amount(), 1);
        assert_ne!(
            existing_account.get_public_key().to_string(),
            ACCOUNT_PUBLIC_KEY
        );

        let new_account = accounts.last().unwrap();
        assert_eq!(new_account.get_amount(), 1000);
        assert_eq!(new_account.get_public_key().to_string(), ACCOUNT_PUBLIC_KEY);
    }
}
