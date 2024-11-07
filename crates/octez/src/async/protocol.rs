pub use super::bootstrap::{
    BootstrapAccount, BootstrapContract, BootstrapSmartRollup, SmartRollupPvmKind,
};
use super::bootstrap::{BootstrapAccounts, BootstrapContracts, BootstrapSmartRollups};

use rust_embed::Embed;
use serde_json::Value;
use std::fmt::Display;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub trait ReadWritable: Read + Write {
    fn path(&self) -> PathBuf;
}

impl ReadWritable for tempfile::NamedTempFile {
    fn path(&self) -> PathBuf {
        PathBuf::from(self.path())
    }
}

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

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Protocol {
    Alpha,
    ParisC,
    Quebec,
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
            Protocol::ParisC => "PsParisCZo7KAh1Z1smVd9ZMZ1HHn5gkzbM94V3PLCpknFWhUAi",
            Protocol::Quebec => "PsQubecQubecQubecQubecQubecQubecQubecQubecQubec",
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
    /// Bootstrap contracts.
    bootstrap_contracts: BootstrapContracts,
    /// Bootstrap smart rollups.
    bootstrap_smart_rollups: BootstrapSmartRollups,
    /// Path to an existing parameter file whose content will be used as the base
    /// parameter set. If `source_path` is not given, a predefined parameter
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
        accounts: impl IntoIterator<Item = BootstrapAccount>,
    ) -> &mut Self {
        self.bootstrap_accounts = BootstrapAccounts::default();
        for account in accounts {
            self.bootstrap_accounts.add_account(account);
        }
        self
    }

    pub fn set_bootstrap_contracts(
        &mut self,
        contracts: impl IntoIterator<Item = BootstrapContract>,
    ) -> &mut Self {
        self.bootstrap_contracts = BootstrapContracts::default();
        for contract in contracts {
            self.bootstrap_contracts.add_contract(contract);
        }
        self
    }

    pub fn set_bootstrap_smart_rollups(
        &mut self,
        rollups: impl IntoIterator<Item = BootstrapSmartRollup>,
    ) -> &mut Self {
        self.bootstrap_smart_rollups = BootstrapSmartRollups::default();
        for rollup in rollups {
            self.bootstrap_smart_rollups.add_rollup(rollup);
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

        self.merge_bootstrap_accounts(json)?;
        self.bootstrap_accounts = BootstrapAccounts::default();
        self.merge_bootstrap_contracts(json)?;
        self.bootstrap_contracts = BootstrapContracts::default();
        self.merge_bootstrap_smart_rollups(json)?;
        self.bootstrap_smart_rollups = BootstrapSmartRollups::default();

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

    fn merge_bootstrap_accounts(
        &mut self,
        json: &mut serde_json::Map<String, Value>,
    ) -> anyhow::Result<()> {
        let mut accounts = BootstrapAccounts::default();
        if let Some(value) = json.get("bootstrap_accounts") {
            let existing_accounts = serde_json::from_value(value.clone())?;
            accounts.merge(&existing_accounts);
        }
        accounts.merge(&self.bootstrap_accounts);
        json.insert(
            "bootstrap_accounts".to_owned(),
            serde_json::to_value(accounts)?,
        );
        Ok(())
    }

    fn merge_bootstrap_contracts(
        &mut self,
        json: &mut serde_json::Map<String, Value>,
    ) -> anyhow::Result<()> {
        let mut contracts = BootstrapContracts::default();
        if let Some(value) = json.get("bootstrap_contracts") {
            let existing_contracts = serde_json::from_value(value.clone())?;
            contracts.merge(&existing_contracts);
        }
        contracts.merge(&self.bootstrap_contracts);
        json.insert(
            "bootstrap_contracts".to_owned(),
            serde_json::to_value(contracts)?,
        );
        Ok(())
    }

    fn merge_bootstrap_smart_rollups(
        &mut self,
        json: &mut serde_json::Map<String, Value>,
    ) -> anyhow::Result<()> {
        let mut rollups = BootstrapSmartRollups::default();
        if let Some(value) = json.get("bootstrap_smart_rollups") {
            let existing_rollups = serde_json::from_value(value.clone())?;
            rollups.merge(&existing_rollups);
        }
        rollups.merge(&self.bootstrap_smart_rollups);
        json.insert(
            "bootstrap_smart_rollups".to_owned(),
            serde_json::to_value(rollups)?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::io::{Seek, Write};

    use tempfile::NamedTempFile;

    use super::{
        BootstrapAccount, BootstrapContract, BootstrapSmartRollup, Protocol,
        ProtocolConstants, ProtocolParameterBuilder, SmartRollupPvmKind,
    };

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";
    const CONTRACT_HASH: &str = "KT1QuofAgnsWffHzLA7D78rxytJruGHDe7XG";
    const SMART_ROLLUP_ADDRESS: &str = "sr1Upj1Zguseor6FdP6mMGgf7VoYxEVQvNZX";

    fn create_dummy_source_file(
        bootstrap_accounts: Option<Vec<BootstrapAccount>>,
        bootstrap_contracts: Option<Vec<BootstrapContract>>,
        bootstrap_rollups: Option<Vec<BootstrapSmartRollup>>,
    ) -> NamedTempFile {
        let mut source_file = tempfile::NamedTempFile::new().unwrap();
        let mut json = serde_json::json!({"foo":"bar"});
        if let Some(accounts) = bootstrap_accounts {
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "bootstrap_accounts".to_owned(),
                Value::Array(
                    accounts
                        .iter()
                        .map(|v| serde_json::to_value(v).unwrap())
                        .collect::<Vec<Value>>(),
                ),
            );
        }
        if let Some(contracts) = bootstrap_contracts {
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "bootstrap_contracts".to_owned(),
                Value::Array(
                    contracts
                        .iter()
                        .map(|v| serde_json::to_value(v).unwrap())
                        .collect::<Vec<Value>>(),
                ),
            );
        }
        if let Some(rollups) = bootstrap_rollups {
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "bootstrap_smart_rollups".to_owned(),
                Value::Array(
                    rollups
                        .iter()
                        .map(|v| serde_json::to_value(v).unwrap())
                        .collect::<Vec<Value>>(),
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
        let account = BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap();
        let contract =
            BootstrapContract::new(serde_json::json!("foobar"), 0, Some(CONTRACT_HASH))
                .unwrap();
        let rollup = BootstrapSmartRollup::new(
            SMART_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Riscv,
            "dummy-kernel",
            serde_json::json!("dummy-params"),
        )
        .unwrap();
        builder
            .set_constants(ProtocolConstants::Sandbox)
            .set_protocol(Protocol::Alpha)
            .set_source_path("/test/path")
            .set_bootstrap_accounts([account.clone()])
            .set_bootstrap_contracts([contract.clone()])
            .set_bootstrap_smart_rollups([rollup.clone()]);
        assert_eq!(builder.constants.unwrap(), ProtocolConstants::Sandbox);
        assert_eq!(builder.source_path.unwrap().to_str().unwrap(), "/test/path");
        assert_eq!(builder.protocol.unwrap().hash(), Protocol::Alpha.hash());
        assert_eq!(builder.bootstrap_accounts.accounts().len(), 1);
        assert_eq!(
            *builder.bootstrap_accounts.accounts().last().unwrap(),
            &account
        );
        let contracts = builder.bootstrap_contracts.contracts();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts.last().unwrap(), &contract);
        let rollups = builder.bootstrap_smart_rollups.rollups();
        assert_eq!(rollups.len(), 1);
        assert_eq!(*rollups.last().unwrap(), &rollup);
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
        let source_file = create_dummy_source_file(None, None, None);
        builder.set_source_path(source_file.path().to_str().unwrap());

        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        // this output file should have the values as the source file above
        assert_eq!(json.get("foo").unwrap().as_str().unwrap(), "bar");
    }

    #[test]
    fn set_bootstrap_accounts() {
        let mut builder = ProtocolParameterBuilder::new();
        builder.set_bootstrap_accounts([
            BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()
        ]);
        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        let accounts = json.get("bootstrap_accounts").unwrap().as_array().unwrap();
        assert_eq!(accounts.len(), 1);
        let account = accounts.last().unwrap().as_array().unwrap();
        assert_eq!(
            account.first().unwrap().as_str().unwrap(),
            ACCOUNT_PUBLIC_KEY
        );
        assert_eq!(account.get(1).unwrap().as_str().unwrap(), "1000");
    }

    #[test]
    fn merge_existing_bootstrap_accounts() {
        let mut builder = ProtocolParameterBuilder::new();
        let accounts = [
            BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1).unwrap(),
            BootstrapAccount::new(
                "edpkvLEsnuq1TnYX9uc4Mcig9AiP7m3VtHpNGViDBivbvYwzzhzZRx",
                1000,
            )
            .unwrap(),
            BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap(),
        ];
        let source_file =
            create_dummy_source_file(Some(vec![accounts[0].clone()]), None, None);
        builder
            .set_source_path(source_file.path().to_str().unwrap())
            .set_bootstrap_accounts(accounts[1..].to_vec());
        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        let mut accounts = json
            .get("bootstrap_accounts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect::<Vec<BootstrapAccount>>();
        assert_eq!(accounts.len(), 2);
        accounts.sort_by_key(|v| v.amount());

        // accounts sorted by tez in ascending order, so the first one is the existing account
        // with 1 mutez only. The 3rd account should not overwrite the 1st account because the
        // 1st account was added in the source param file
        let first_account = accounts.first().unwrap();
        assert_eq!(first_account, &accounts[0]);

        let second_account = accounts.last().unwrap();
        assert_eq!(second_account, &accounts[1]);
    }

    #[test]
    fn set_bootstrap_contracts() {
        let mut builder = ProtocolParameterBuilder::new();
        builder.set_bootstrap_contracts([
            BootstrapContract::new(serde_json::json!("foobar"), 1000, None).unwrap(),
            BootstrapContract::new(
                serde_json::json!("foobar"),
                2000,
                Some(CONTRACT_HASH),
            )
            .unwrap(),
        ]);
        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        let mut contracts = json
            .get("bootstrap_contracts")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(contracts.len(), 2);
        contracts.sort_by_key(|v| v.get("amount").unwrap().as_str().unwrap().to_owned());

        let contract = contracts.first().unwrap();
        assert_eq!(
            contract,
            &serde_json::json!({"amount": "1000", "script": "foobar"})
        );

        let contract = contracts.last().unwrap();
        assert_eq!(
            contract,
            &serde_json::json!({"hash": CONTRACT_HASH, "amount": "2000", "script": "foobar"})
        );
    }

    #[test]
    fn merge_existing_bootstrap_contracts() {
        let contracts = [
            BootstrapContract::new(
                serde_json::json!("existing-contract"),
                100,
                Some(CONTRACT_HASH),
            )
            .unwrap(),
            BootstrapContract::new(
                serde_json::json!("new-contract"),
                1000,
                Some("KT1L7KRpTBC4jqBVAuNdjcscp2jpC3xaogzK"),
            )
            .unwrap(),
            BootstrapContract::new(
                serde_json::json!("skipped-contract"),
                1000,
                Some(CONTRACT_HASH),
            )
            .unwrap(),
        ];
        let mut builder = ProtocolParameterBuilder::new();
        let source_file =
            create_dummy_source_file(None, Some(vec![contracts[0].clone()]), None);
        builder
            .set_source_path(source_file.path().to_str().unwrap())
            .set_bootstrap_contracts(contracts[1..].to_vec());
        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        let contracts = json
            .get("bootstrap_contracts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect::<Vec<BootstrapContract>>();
        assert_eq!(contracts.len(), 2);

        // the 3rd contract was not injected because its hash collides with the 1st contract
        let existing_contract = contracts.first().unwrap();
        assert_eq!(existing_contract, &contracts[0]);
        let new_contract = contracts.last().unwrap();
        assert_eq!(new_contract, &contracts[1]);
    }

    #[test]
    fn set_bootstrap_smart_rollups() {
        let mut builder = ProtocolParameterBuilder::new();
        let rollup = BootstrapSmartRollup::new(
            SMART_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Riscv,
            "dummy-kernel",
            serde_json::json!("dummy-params"),
        )
        .unwrap();
        builder.set_bootstrap_smart_rollups([rollup.clone()]);
        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();

        let rollups = json
            .get("bootstrap_smart_rollups")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(rollups.len(), 1);
        let found_rollup = rollups.last().unwrap();
        assert_eq!(
            found_rollup,
            &serde_json::json!({
                "address": SMART_ROLLUP_ADDRESS,
                "pvm_kind": SmartRollupPvmKind::Riscv,
                "kernel": "dummy-kernel",
                "parameters_ty": "dummy-params"
            })
        );
    }

    #[test]
    fn merge_existing_bootstrap_rollups() {
        let first_rollup = BootstrapSmartRollup::new(
            SMART_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Riscv,
            "foo-kernel",
            serde_json::json!("foo-params"),
        )
        .unwrap();
        let skipped_rollup = BootstrapSmartRollup::new(
            SMART_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Riscv,
            "bar-kernel",
            serde_json::Value::String("bar-params".to_owned()),
        )
        .unwrap();
        let second_rollup = BootstrapSmartRollup::new(
            "sr1Ghq66tYK9y3r8CC1Tf8i8m5nxh8nTvZEf",
            SmartRollupPvmKind::Riscv,
            "new-kernel",
            serde_json::Value::String("new-params".to_owned()),
        )
        .unwrap();

        let mut builder = ProtocolParameterBuilder::new();
        let source_file =
            create_dummy_source_file(None, None, Some(vec![first_rollup.clone()]));
        builder
            .set_source_path(source_file.path().to_str().unwrap())
            .set_bootstrap_smart_rollups(vec![
                skipped_rollup.clone(),
                second_rollup.clone(),
            ]);

        let output_file = builder.build().unwrap();
        let json: Value = serde_json::from_reader(output_file).unwrap();
        let mut rollups = json
            .get("bootstrap_smart_rollups")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect::<Vec<BootstrapSmartRollup>>();
        assert_eq!(rollups.len(), 2);
        rollups.sort_by_key(|v| v.kernel().to_owned());
        assert_eq!(rollups.first().unwrap(), &first_rollup);
        assert_eq!(rollups.last().unwrap(), &second_rollup);
    }
}
