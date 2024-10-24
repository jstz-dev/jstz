use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use jstz_crypto::public_key::PublicKey;
use serde_json::Value;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapAccount {
    public_key: PublicKey,
    amount_mutez: u64,
}

impl TryFrom<&Value> for BootstrapAccount {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let array = value
            .as_array()
            .ok_or(anyhow::anyhow!("value is not a valid json array"))?;
        if array.len() != 2 {
            return Err(anyhow::anyhow!(
                "value is not in the acceptable format for bootstrap accounts"
            ));
        }
        let public_key = array
            .first()
            .unwrap()
            .as_str()
            .ok_or(anyhow::anyhow!("'public_key' is not a valid string"))?;
        let amount_mutez = array
            .get(1)
            .unwrap()
            .as_str()
            .ok_or(anyhow::anyhow!(
                "'amount' is not a valid string representing an unsigned integer"
            ))?
            .parse::<u64>();
        if amount_mutez.is_err() {
            return Err(anyhow::anyhow!(
                "'amount' is not a valid string representing an unsigned integer"
            ));
        }
        Ok(Self {
            public_key: PublicKey::from_base58(public_key)?,
            amount_mutez: amount_mutez.unwrap(),
        })
    }
}

impl From<&BootstrapAccount> for Value {
    fn from(value: &BootstrapAccount) -> Self {
        Value::Array(vec![
            Value::String(value.public_key.to_string()),
            Value::String(value.amount_mutez.to_string()),
        ])
    }
}

impl BootstrapAccount {
    pub fn new(public_key: &str, amount_mutez: u64) -> anyhow::Result<Self> {
        Ok(Self {
            public_key: PublicKey::from_base58(public_key)?,
            amount_mutez,
        })
    }

    #[cfg(test)]
    pub fn amount(&self) -> u64 {
        self.amount_mutez
    }
}

#[derive(Default)]
pub struct BootstrapAccounts {
    accounts: HashMap<String, BootstrapAccount>,
}

impl BootstrapAccounts {
    pub fn add_account(&mut self, account: BootstrapAccount) {
        let key = account.public_key.to_string();
        self.accounts.entry(key).or_insert(account);
    }

    pub fn accounts(&self) -> Vec<&BootstrapAccount> {
        self.accounts.values().collect::<Vec<&BootstrapAccount>>()
    }
}

impl From<&BootstrapAccounts> for Value {
    fn from(value: &BootstrapAccounts) -> Self {
        Value::Array(value.accounts.values().map(Value::from).collect())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapContract {
    script: Value,
    amount_mutez: u64,
    hash: Option<ContractKt1Hash>,
}

impl TryFrom<&Value> for BootstrapContract {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let map = value
            .as_object()
            .ok_or(anyhow::anyhow!("value is not a valid json object"))?;
        let hash = match map.get("hash") {
            Some(v) => Some(ContractKt1Hash::from_base58_check(
                v.as_str()
                    .ok_or(anyhow::anyhow!("'hash' is not a valid string"))?,
            )?),
            None => None,
        };
        let amount_mutez = map
            .get("amount")
            .ok_or(anyhow::anyhow!("'amount' is missing from the given value"))?
            .as_str()
            .ok_or(anyhow::anyhow!(
                "'amount' is not a valid string representing an unsigned integer"
            ))?
            .parse::<u64>();
        if amount_mutez.is_err() {
            return Err(anyhow::anyhow!(
                "'amount' is not a valid string representing an unsigned integer"
            ));
        }
        let script = map
            .get("script")
            .ok_or(anyhow::anyhow!("'script' is missing from the given value"))?
            .clone();
        Ok(Self {
            hash,
            script,
            amount_mutez: amount_mutez.unwrap(),
        })
    }
}

impl From<&BootstrapContract> for Value {
    fn from(value: &BootstrapContract) -> Self {
        Value::Object({
            let mut map = serde_json::Map::new();
            map.insert("script".to_owned(), value.script.clone());
            map.insert(
                "amount".to_owned(),
                Value::String(value.amount_mutez.to_string()),
            );
            if let Some(v) = &value.hash {
                map.insert("hash".to_owned(), Value::String(v.to_string()));
            }
            map
        })
    }
}

impl BootstrapContract {
    pub fn new(
        script: Value,
        amount_mutez: u64,
        hash: Option<&str>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            script,
            amount_mutez,
            hash: match hash {
                Some(v) => Some(ContractKt1Hash::from_base58_check(v)?),
                None => None,
            },
        })
    }
}

#[derive(Default)]
pub struct BootstrapContracts {
    keys: HashSet<String>,
    contracts: Vec<BootstrapContract>,
}

impl From<&BootstrapContracts> for Value {
    fn from(value: &BootstrapContracts) -> Self {
        Value::Array(value.contracts.iter().map(Value::from).collect())
    }
}

impl BootstrapContracts {
    pub fn add_contract(&mut self, contract: BootstrapContract) {
        if let Some(hash) = &contract.hash {
            let key = hash.to_string();
            if self.keys.contains(&key) {
                return;
            }
            self.keys.insert(key);
        }
        self.contracts.push(contract);
    }

    pub fn contracts(&self) -> &Vec<BootstrapContract> {
        &self.contracts
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SmartRollupPvmKind {
    Wasm,
    Arith,
    Riscv,
}

impl Display for SmartRollupPvmKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Wasm => "wasm_2_0_0",
            Self::Arith => "arith",
            Self::Riscv => "riscv",
        })
    }
}

impl TryFrom<&str> for SmartRollupPvmKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<SmartRollupPvmKind> {
        match value {
            "wasm_2_0_0" => Ok(Self::Wasm),
            "arith" => Ok(Self::Arith),
            "riscv" => Ok(Self::Riscv),
            _ => Err(anyhow::anyhow!("Unknown PVM type '{}'", value)),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapSmartRollup {
    /// Rollup address.
    address: SmartRollupHash,
    /// Type of Proof-generating Virtual Machine (PVM) that interprets the kernel.
    pvm_kind: SmartRollupPvmKind,
    /// Smart rollup kernel in hex string.
    kernel: String,
    /// Michelson type of the rollup entrypoint in JSON format.
    parameters_ty: Value,
}

impl TryFrom<&Value> for BootstrapSmartRollup {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let map = value
            .as_object()
            .ok_or(anyhow::anyhow!("value is not a valid json object"))?;
        let address = SmartRollupHash::from_base58_check(
            map.get("address")
                .ok_or(anyhow::anyhow!("'address' is missing from the given value"))?
                .as_str()
                .ok_or(anyhow::anyhow!("'address' is not a valid string"))?,
        )?;
        let pvm_kind = SmartRollupPvmKind::try_from(
            map.get("pvm_kind")
                .ok_or(anyhow::anyhow!(
                    "'pvm_kind' is missing from the given value"
                ))?
                .as_str()
                .ok_or(anyhow::anyhow!("'pvm_kind' is not a valid string"))?,
        )?;
        let kernel = map
            .get("kernel")
            .ok_or(anyhow::anyhow!("'kernel' is missing from the given value"))?
            .as_str()
            .ok_or(anyhow::anyhow!("'kernel' is not a valid string"))?
            .to_owned();
        Ok(Self {
            address,
            pvm_kind,
            kernel,
            parameters_ty: map
                .get("parameters_ty")
                .ok_or(anyhow::anyhow!(
                    "'parameters_ty' is missing from the given value"
                ))?
                .clone(),
        })
    }
}

impl From<&BootstrapSmartRollup> for Value {
    fn from(value: &BootstrapSmartRollup) -> Self {
        Value::Object({
            let mut map = serde_json::Map::new();
            map.insert("parameters_ty".to_owned(), value.parameters_ty.clone());
            map.insert("kernel".to_owned(), Value::String(value.kernel.clone()));
            map.insert(
                "pvm_kind".to_owned(),
                Value::String(value.pvm_kind.to_string()),
            );
            map.insert(
                "address".to_owned(),
                Value::String(value.address.to_string()),
            );
            map
        })
    }
}

impl BootstrapSmartRollup {
    pub fn new(
        address: &str,
        pvm_kind: SmartRollupPvmKind,
        kernel: &str,
        parameters_ty: Value,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            address: SmartRollupHash::from_base58_check(address)?,
            pvm_kind,
            kernel: kernel.to_owned(),
            parameters_ty,
        })
    }

    #[cfg(test)]
    pub fn kernel(&self) -> &str {
        &self.kernel
    }
}

#[derive(Default)]
pub struct BootstrapSmartRollups {
    rollups: HashMap<String, BootstrapSmartRollup>,
}

impl From<&BootstrapSmartRollups> for Value {
    fn from(value: &BootstrapSmartRollups) -> Self {
        Value::Array(value.rollups.values().map(Value::from).collect())
    }
}

impl BootstrapSmartRollups {
    pub fn add_rollup(&mut self, rollup: BootstrapSmartRollup) {
        let key = rollup.address.to_string();
        self.rollups.entry(key).or_insert(rollup);
    }

    pub fn rollups(&self) -> Vec<&BootstrapSmartRollup> {
        self.rollups
            .values()
            .collect::<Vec<&BootstrapSmartRollup>>()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{
        BootstrapAccount, BootstrapAccounts, BootstrapContract, BootstrapContracts,
        BootstrapSmartRollup, BootstrapSmartRollups, SmartRollupPvmKind,
    };
    use serde_json::Value;

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";
    const CONTRACT_HASH: &str = "KT1QuofAgnsWffHzLA7D78rxytJruGHDe7XG";
    const SMART_ROLLUP_ADDRESS: &str = "sr1Upj1Zguseor6FdP6mMGgf7VoYxEVQvNZX";

    #[test]
    fn bootstrap_account_new() {
        let account = BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap();
        assert_eq!(account.public_key.to_string(), ACCOUNT_PUBLIC_KEY);
        assert_eq!(account.amount_mutez, 1000);
    }

    #[test]
    fn bootstrap_account_try_from() {
        let value = serde_json::json!([ACCOUNT_PUBLIC_KEY, "1"]);
        let account = BootstrapAccount::try_from(&value).unwrap();
        assert_eq!(account.amount_mutez, 1);
        assert_eq!(account.public_key.to_string(), ACCOUNT_PUBLIC_KEY);

        let value = serde_json::json!([ACCOUNT_PUBLIC_KEY, "-1"]);
        BootstrapAccount::try_from(&value)
            .expect_err("try_from should fail with negative amount");
    }

    #[test]
    fn bootstrap_accounts_add_duplicates() {
        let mut accounts = BootstrapAccounts::default();
        accounts.add_account(BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap());
        assert_eq!(accounts.accounts.len(), 1);
        accounts.add_account(BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap());
        assert_eq!(accounts.accounts.len(), 1);
        accounts.add_account(
            BootstrapAccount::new(
                "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
                1000,
            )
            .unwrap(),
        );
        assert_eq!(accounts.accounts.len(), 2);
    }

    #[test]
    fn serde_value_from_bootstrap_accounts() {
        let accounts = BootstrapAccounts {
            accounts: HashMap::from_iter([(
                ACCOUNT_PUBLIC_KEY.to_owned(),
                BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap(),
            )]),
        };
        let value = Value::from(&accounts);
        assert_eq!(value, serde_json::json!([[ACCOUNT_PUBLIC_KEY, "1000"]]));
    }

    #[test]
    fn bootstrap_contract_new() {
        let contract = BootstrapContract::new(
            Value::String("dummy-script".to_owned()),
            1000,
            Some(CONTRACT_HASH),
        )
        .unwrap();
        assert_eq!(contract.amount_mutez, 1000);
        assert_eq!(contract.hash.unwrap().to_string(), CONTRACT_HASH);
        assert_eq!(contract.script.as_str().unwrap(), "dummy-script");
    }

    #[test]
    fn bootstrap_contracts_add_contracts() {
        let mut contracts = BootstrapContracts::default();
        let contract =
            BootstrapContract::new(serde_json::json!("foo"), 1000, Some(CONTRACT_HASH))
                .unwrap();
        contracts.add_contract(contract.clone());
        assert_eq!(contracts.contracts().len(), 1);
        contracts.add_contract(contract);
        assert_eq!(contracts.contracts().len(), 1);
        contracts.add_contract(
            BootstrapContract::new(
                serde_json::json!("foo"),
                1000,
                Some("KT19e6TBL5dNQ29gtaQNPnJfwYHsbCpGyn7d"),
            )
            .unwrap(),
        );
        assert_eq!(contracts.contracts().len(), 2);
        contracts.add_contract(
            BootstrapContract::new(serde_json::json!("foo"), 1000, None).unwrap(),
        );
        assert_eq!(contracts.contracts().len(), 3);
    }

    #[test]
    fn bootstrap_contract_try_from_serde_value() {
        let json = serde_json::json!({"script": "foobar", "amount": "1000", "hash": CONTRACT_HASH});
        let contract = BootstrapContract::try_from(&json).unwrap();
        assert_eq!(
            contract,
            BootstrapContract::new(
                Value::String("foobar".to_owned()),
                1000,
                Some(CONTRACT_HASH),
            )
            .unwrap()
        )
    }

    #[test]
    fn serde_value_from_bootstrap_contracts() {
        let contracts = BootstrapContracts {
            keys: HashSet::from_iter([CONTRACT_HASH.to_owned()]),
            contracts: vec![
                BootstrapContract::new(
                    Value::String("dummy-script".to_owned()),
                    1000,
                    Some(CONTRACT_HASH),
                )
                .unwrap(),
                BootstrapContract::new(
                    Value::String("dummy-script-no-hash".to_owned()),
                    900,
                    None,
                )
                .unwrap(),
            ],
        };
        let value = Value::from(&contracts);
        assert_eq!(
            value,
            serde_json::json!([{"amount": "1000", "script": "dummy-script", "hash": CONTRACT_HASH}, {"amount":"900", "script": "dummy-script-no-hash"}])
        );
    }

    #[test]
    fn bootstrap_smart_rollup_new() {
        let rollup = BootstrapSmartRollup::new(
            SMART_ROLLUP_ADDRESS,
            SmartRollupPvmKind::Riscv,
            "dummy-kernel",
            Value::String("dummy-params".to_owned()),
        )
        .unwrap();
        assert_eq!(rollup.address.to_string(), SMART_ROLLUP_ADDRESS);
        assert_eq!(rollup.pvm_kind, SmartRollupPvmKind::Riscv);
        assert_eq!(rollup.kernel, "dummy-kernel");
        assert_eq!(rollup.parameters_ty.as_str().unwrap(), "dummy-params");
    }

    #[test]
    fn serde_value_from_bootstrap_smart_rollups() {
        let rollups = BootstrapSmartRollups {
            rollups: HashMap::from_iter([(
                SMART_ROLLUP_ADDRESS.to_owned(),
                BootstrapSmartRollup::new(
                    SMART_ROLLUP_ADDRESS,
                    SmartRollupPvmKind::Riscv,
                    "dummy-kernel",
                    Value::String("dummy-params".to_owned()),
                )
                .unwrap(),
            )]),
        };
        let value = Value::from(&rollups);
        assert_eq!(
            value,
            serde_json::json!([{"address": SMART_ROLLUP_ADDRESS, "pvm_kind": "riscv", "kernel": "dummy-kernel", "parameters_ty": "dummy-params"}])
        );
    }

    #[test]
    fn pvm_kind_fmt() {
        assert_eq!(SmartRollupPvmKind::Arith.to_string(), "arith");
        assert_eq!(SmartRollupPvmKind::Riscv.to_string(), "riscv");
        assert_eq!(SmartRollupPvmKind::Wasm.to_string(), "wasm_2_0_0");
    }

    #[test]
    fn pvm_kind_from_str() {
        assert_eq!(
            SmartRollupPvmKind::try_from("arith").unwrap(),
            SmartRollupPvmKind::Arith
        );
        assert_eq!(
            SmartRollupPvmKind::try_from("riscv").unwrap(),
            SmartRollupPvmKind::Riscv
        );
        assert_eq!(
            SmartRollupPvmKind::try_from("wasm_2_0_0").unwrap(),
            SmartRollupPvmKind::Wasm
        );
        assert_eq!(
            SmartRollupPvmKind::try_from("dummy")
                .unwrap_err()
                .to_string(),
            "Unknown PVM type 'dummy'"
        )
    }
}
