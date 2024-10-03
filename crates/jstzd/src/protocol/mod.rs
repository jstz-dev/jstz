mod bootstrap;

pub use bootstrap::BootstrapAccount;
use bootstrap::BootstrapAccounts;
use rust_embed::Embed;
use serde_json::Value;
use std::fmt::Display;
use std::path::{Path, PathBuf};

#[derive(PartialEq, Eq, Debug)]
pub enum ProtocolConstants {
    Sandbox,
    #[cfg(test)]
    TestConstants,
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
            #[cfg(test)]
            Self::TestConstants => "test-test",
        })
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Protocol {
    Alpha,
    #[cfg(test)]
    TestVersion,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Alpha
    }
}

impl Protocol {
    fn hash(self) -> &'static str {
        match self {
            Protocol::Alpha => "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            #[cfg(test)]
            Protocol::TestVersion => "test-hash",
        }
    }

    fn parameter_file(self, constants: ProtocolConstants) -> PathBuf {
        Path::new(&constants.to_string()).join(self.hash())
    }
}

#[cfg(not(test))]
#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/protocol_parameters/"]
pub struct ProtocolParameterFile;

#[cfg(test)]
#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/test/protocol_parameters/"]
pub struct ProtocolParameterFile;

#[derive(Default)]
pub struct ProtocolParameterBuilder {
    protocol: Protocol,
    constants: ProtocolConstants,
    bootstrap_accounts: BootstrapAccounts,
    path: Option<PathBuf>,
}

impl ProtocolParameterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_protocol(&mut self, protocol: Protocol) -> &mut Self {
        self.protocol = protocol;
        self
    }

    pub fn set_constants(&mut self, constants: ProtocolConstants) -> &mut Self {
        self.constants = constants;
        self
    }

    pub fn set_bootstrap_accounts(
        &mut self,
        accounts: Vec<BootstrapAccount>,
    ) -> &mut Self {
        self.bootstrap_accounts.accounts = accounts;
        self
    }

    pub fn set_path(&mut self, path: &str) -> &mut Self {
        self.path = Some(PathBuf::from(path));
        self
    }

    pub async fn build(self) -> anyhow::Result<PathBuf> {
        let f = ProtocolParameterFile::get(
            self.protocol
                .parameter_file(self.constants)
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let mut raw_json: Value = serde_json::from_slice(&f.data)?;
        let json = raw_json
            .as_object_mut()
            .ok_or(anyhow::anyhow!("Failed to convert json file"))?;
        json.insert(
            "bootstrap_accounts".to_owned(),
            Value::from(self.bootstrap_accounts),
        );
        drop(f);
        let path = self
            .path
            .unwrap_or(tempfile::NamedTempFile::new().unwrap().path().to_path_buf());
        let file = std::fs::File::create(&path).unwrap();
        serde_json::to_writer(file, &json)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BootstrapAccount, Protocol, ProtocolConstants, ProtocolParameterBuilder,
    };

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpktzB3sirfeX6PrgAgWvRVT8Fd28jVLbWXKJmaUrYmK2UoSHc1eJ";

    #[test]
    fn parameter_builder() {
        let mut builder = ProtocolParameterBuilder::new();
        builder
            .set_constants(ProtocolConstants::TestConstants)
            .set_protocol(Protocol::TestVersion)
            .set_path("/test/path")
            .set_bootstrap_accounts(
                [BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()].to_vec(),
            );
        assert_eq!(builder.constants, ProtocolConstants::TestConstants);
        assert_eq!(
            builder.path.unwrap().as_os_str().to_str().unwrap(),
            "/test/path"
        );
        assert_eq!(builder.protocol.hash(), Protocol::TestVersion.hash());
        assert_eq!(builder.bootstrap_accounts.accounts.len(), 1);
        assert_eq!(
            builder.bootstrap_accounts.accounts.last().unwrap(),
            &BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()
        );
    }

    #[test]
    fn parameter_builder_default() {
        let builder = ProtocolParameterBuilder::new();
        assert_eq!(
            builder.constants.to_string(),
            ProtocolConstants::Sandbox.to_string(),
        );
        assert!(builder.path.is_none());
        assert_eq!(builder.protocol, Protocol::Alpha);
        assert!(builder.bootstrap_accounts.accounts.is_empty());
    }

    #[tokio::test]
    async fn build_protocol_parameter() {
        let mut builder = ProtocolParameterBuilder::new();
        let output_file = tempfile::NamedTempFile::new().unwrap();
        let expected_output_path = output_file.path();
        builder
            .set_path(expected_output_path.as_os_str().to_str().unwrap())
            .set_protocol(Protocol::Alpha)
            .set_constants(ProtocolConstants::Sandbox)
            .set_bootstrap_accounts(
                [BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()].to_vec(),
            );
        let output_path = builder.build().await.unwrap();
        assert_eq!(expected_output_path, output_path);
        let file = std::fs::File::open(output_path).unwrap();
        let json: serde_json::Value = serde_json::from_reader(file).unwrap();
        assert_eq!(
            json.get("consensus_rights_delay")
                .unwrap()
                .as_u64()
                .unwrap(),
            2
        );

        // Check accounts
        let accounts = json.get("bootstrap_accounts").unwrap().as_array().unwrap();
        assert_eq!(accounts.len(), 1);
        let account = accounts.last().unwrap().as_object().unwrap();
        let mut keys = account.keys().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, ["amount", "public_key"]);
        assert_eq!(account.get("amount").unwrap(), 1000);
        assert_eq!(account.get("public_key").unwrap(), ACCOUNT_PUBLIC_KEY);
    }
}
