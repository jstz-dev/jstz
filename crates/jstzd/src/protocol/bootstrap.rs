use jstz_crypto::public_key::PublicKey;
use serde_json::Value;
use tezos_crypto_rs::hash::ContractKt1Hash;

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

#[cfg(test)]
mod tests {
    use super::{
        BootstrapAccount, BootstrapAccounts, BootstrapContract, BootstrapContracts,
    };
    use serde_json::Value;

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";
    const CONTRACT_HASH: &str = "KT1QuofAgnsWffHzLA7D78rxytJruGHDe7XG";

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
}
