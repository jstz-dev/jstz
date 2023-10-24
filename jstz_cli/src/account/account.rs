use anyhow::Result;
use jstz_crypto::{
    keypair_from_passphrase, public_key::PublicKey, public_key_hash::PublicKeyHash,
    secret_key::SecretKey,
};
use serde::{Deserialize, Serialize};

fn create_address(pk: &PublicKey) -> String {
    PublicKeyHash::try_from(pk).unwrap().to_base58()
}

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub alias: String,
    pub address: String,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

impl Account {
    pub fn from_passphrase(passphrase: String, alias: String) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str()).unwrap();

        println!("Secret key: {}", sk.to_string());
        println!("Public key: {}", pk.to_string());

        let address = create_address(&pk);
        let new_account = Account {
            alias: alias,
            address: address,
            secret_key: sk,
            public_key: pk,
        };

        Ok(new_account)
    }
}
