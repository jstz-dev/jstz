use jstz_crypto::public_key::PublicKey;
use serde_json::Value;

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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{BootstrapAccount, BootstrapAccounts};

    const ACCOUNT_PUBLIC_KEY: &str =
        "edpkubRfnPoa6ts5vBdTB5syvjeK2AyEF3kyhG4Sx7F9pU3biku4wv";

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
}
