use anyhow::anyhow;
use anyhow::Result;
use derive_more::From;
use jstz_crypto::{
    keypair_from_passphrase, public_key::PublicKey, public_key_hash::PublicKeyHash,
    secret_key::SecretKey,
};
use serde::{Deserialize, Serialize};

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone, From)]
pub enum Account {
    Owned(OwnedAccount),
    Alias(AliasAccount),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OwnedAccount {
    pub alias: String,
    pub address: PublicKeyHash,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

impl OwnedAccount {
    pub fn new(passphrase: String, alias: String) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str()).unwrap();

        let address = PublicKeyHash::try_from(&pk)?;
        let owned_account = Self {
            alias,
            address,
            secret_key: sk,
            public_key: pk,
        };

        Ok(owned_account)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AliasAccount {
    pub alias: String,
    pub address: PublicKeyHash,
}

impl AliasAccount {
    pub fn new(address: String, name: String) -> Result<Self> {
        let alias_account = Self {
            alias: name,
            address: PublicKeyHash::from_base58(address.as_str())?,
        };

        Ok(alias_account)
    }
}

impl Account {
    pub fn alias(&self) -> &str {
        match self {
            Account::Owned(OwnedAccount { alias, .. }) => alias,
            Account::Alias(AliasAccount { alias, .. }) => alias,
        }
    }
    pub fn address(&self) -> &PublicKeyHash {
        match self {
            Account::Owned(OwnedAccount { address, .. }) => address,
            Account::Alias(AliasAccount { address, .. }) => address,
        }
    }

    pub fn as_owned_mut(&mut self) -> Result<&mut OwnedAccount> {
        match self {
            Account::Owned(owned_account) => Ok(owned_account),
            Account::Alias(_) => Err(anyhow!("Account is not owned")),
        }
    }

    pub fn as_owned(&self) -> Result<&OwnedAccount> {
        match self {
            Account::Owned(owned_account) => Ok(owned_account),
            Account::Alias(_) => Err(anyhow!("Account is not owned")),
        }
    }
}
