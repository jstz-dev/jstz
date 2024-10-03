use std::fmt::Display;

use jstz_crypto::public_key::PublicKey;
use serde_json::Value;
use tezos_crypto_rs::hash::{ContractKt1Hash, SmartRollupHash};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapAccount {
    public_key: PublicKey,
    amount_tez: u64,
}

impl BootstrapAccount {
    pub fn new(public_key: &str, amount: u64) -> anyhow::Result<Self> {
        Ok(Self {
            public_key: PublicKey::from_base58(public_key)?,
            amount_tez: amount,
        })
    }
}

#[derive(Default)]
pub struct BootstrapAccounts {
    pub accounts: Vec<BootstrapAccount>,
}

impl From<BootstrapAccounts> for Value {
    fn from(value: BootstrapAccounts) -> Self {
        Value::Array(
            value
                .accounts
                .iter()
                .map(|v| {
                    Value::Object({
                        let mut map = serde_json::Map::new();
                        map.insert(
                            "public_key".to_owned(),
                            Value::String(v.public_key.to_string()),
                        );
                        map.insert(
                            "amount".to_owned(),
                            Value::Number(v.amount_tez.into()),
                        );
                        map
                    })
                })
                .collect(),
        )
    }
}

#[derive(Clone)]
pub struct BootstrapContract {
    script: Value,
    amount_tez: u64,
    hash: Option<ContractKt1Hash>,
}

impl BootstrapContract {
    pub fn new(script: Value, amount: u64, hash: Option<&str>) -> anyhow::Result<Self> {
        Ok(Self {
            script,
            amount_tez: amount,
            hash: match hash {
                Some(v) => Some(ContractKt1Hash::from_base58_check(v)?),
                None => None,
            },
        })
    }
}

#[derive(Default)]
pub struct BootstrapContracts {
    pub contracts: Vec<BootstrapContract>,
}

impl From<BootstrapContracts> for Value {
    fn from(value: BootstrapContracts) -> Self {
        Value::Array(
            value
                .contracts
                .iter()
                .map(|v| {
                    Value::Object({
                        let mut map = serde_json::Map::new();
                        map.insert("script".to_owned(), v.script.clone());
                        map.insert(
                            "amount".to_owned(),
                            Value::Number(v.amount_tez.into()),
                        );
                        if let Some(v) = &v.hash {
                            map.insert("hash".to_owned(), Value::String(v.to_string()));
                        }
                        map
                    })
                })
                .collect(),
        )
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

#[derive(Clone)]
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
}

#[derive(Default)]
pub struct BootstrapSmartRollups {
    pub rollups: Vec<BootstrapSmartRollup>,
}

impl From<BootstrapSmartRollups> for Value {
    fn from(value: BootstrapSmartRollups) -> Self {
        Value::Array(
            value
                .rollups
                .iter()
                .map(|v| {
                    Value::Object({
                        let mut map = serde_json::Map::new();
                        map.insert("parameters_ty".to_owned(), v.parameters_ty.clone());
                        map.insert("kernel".to_owned(), Value::String(v.kernel.clone()));
                        map.insert(
                            "pvm_kind".to_owned(),
                            Value::String(v.pvm_kind.to_string()),
                        );
                        map.insert(
                            "address".to_owned(),
                            Value::String(v.address.to_string()),
                        );
                        map
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(account.amount_tez, 1000);
    }

    #[test]
    fn serde_value_from_bootstrap_accounts() {
        let accounts = BootstrapAccounts {
            accounts: vec![BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()],
        };
        let value = Value::from(accounts);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let account = arr.last().unwrap().as_object().unwrap();
        assert_eq!(
            account.get("public_key").unwrap().as_str().unwrap(),
            ACCOUNT_PUBLIC_KEY
        );
        assert_eq!(account.get("amount").unwrap().as_u64().unwrap(), 1000);
    }

    #[test]
    fn bootstrap_contract_new() {
        let contract = BootstrapContract::new(
            Value::String("dummy-script".to_owned()),
            1000,
            Some(CONTRACT_HASH),
        )
        .unwrap();
        assert_eq!(contract.amount_tez, 1000);
        assert_eq!(contract.hash.unwrap().to_string(), CONTRACT_HASH);
        assert_eq!(contract.script.as_str().unwrap(), "dummy-script");
    }

    #[test]
    fn serde_value_from_bootstrap_contracts() {
        let contracts = BootstrapContracts {
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
        let value = Value::from(contracts);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let contract = arr.first().unwrap().as_object().unwrap();
        assert_eq!(contract.get("amount").unwrap(), 1000);
        assert_eq!(
            contract.get("script").unwrap().as_str().unwrap(),
            "dummy-script"
        );
        assert_eq!(
            contract.get("hash").unwrap().as_str().unwrap(),
            CONTRACT_HASH
        );

        let contract = arr.last().unwrap().as_object().unwrap();
        assert_eq!(contract.get("amount").unwrap(), 900);
        assert_eq!(
            contract.get("script").unwrap().as_str().unwrap(),
            "dummy-script-no-hash"
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
            rollups: vec![BootstrapSmartRollup::new(
                SMART_ROLLUP_ADDRESS,
                SmartRollupPvmKind::Riscv,
                "dummy-kernel",
                Value::String("dummy-params".to_owned()),
            )
            .unwrap()],
        };
        let value = Value::from(rollups);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let rollup = arr.last().unwrap().as_object().unwrap();
        assert_eq!(rollup.get("address").unwrap(), SMART_ROLLUP_ADDRESS);
        assert_eq!(
            rollup.get("pvm_kind").unwrap().as_str().unwrap(),
            SmartRollupPvmKind::Riscv.to_string()
        );
        assert_eq!(
            rollup.get("kernel").unwrap().as_str().unwrap(),
            "dummy-kernel"
        );
        assert_eq!(
            rollup.get("parameters_ty").unwrap().as_str().unwrap(),
            "dummy-params"
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
