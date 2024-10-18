use std::collections::HashSet;

use jstz_crypto::public_key::PublicKey;
use serde_json::Value;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapAccount {
    public_key: PublicKey,
    amount_mutez: u64,
}

impl BootstrapAccount {
    pub fn get_amount(&self) -> u64 {
        self.amount_mutez
    }

    pub fn get_public_key(&self) -> &PublicKey {
        &self.public_key
    }
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
            .get(0)
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

impl BootstrapAccount {
    pub fn new(public_key: &str, amount_mutez: u64) -> anyhow::Result<Self> {
        Ok(Self {
            public_key: PublicKey::from_base58(public_key)?,
            amount_mutez,
        })
    }
}

#[derive(Default)]
pub struct BootstrapAccounts {
    keys: HashSet<String>,
    accounts: Vec<BootstrapAccount>,
}

impl BootstrapAccounts {
    pub fn add_account(&mut self, account: BootstrapAccount) {
        let key = account.public_key.to_string();
        if !self.keys.contains(&key) {
            self.accounts.push(account);
            self.keys.insert(key);
        }
    }

    pub fn get_accounts(&self) -> &Vec<BootstrapAccount> {
        &self.accounts
    }
}

impl From<&BootstrapAccounts> for Value {
    fn from(value: &BootstrapAccounts) -> Self {
        Value::Array(
            value
                .accounts
                .iter()
                .map(|v| {
                    Value::Array(vec![
                        Value::String(v.public_key.to_string()),
                        Value::String(v.amount_mutez.to_string()),
                    ])
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::Value;

    use super::{BootstrapAccount, BootstrapAccounts};

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";

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
            keys: HashSet::from_iter(vec![ACCOUNT_PUBLIC_KEY.to_owned()]),
            accounts: vec![BootstrapAccount::new(ACCOUNT_PUBLIC_KEY, 1000).unwrap()],
        };
        let value = Value::from(&accounts);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let account = arr.last().unwrap().as_array().unwrap();
        assert_eq!(
            account.get(0).unwrap().as_str().unwrap(),
            ACCOUNT_PUBLIC_KEY
        );
        assert_eq!(account.get(1).unwrap().as_str().unwrap(), "1000");
    }
}
