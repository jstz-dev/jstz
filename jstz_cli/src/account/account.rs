use anyhow::Result;
use jstz_crypto::{
    keypair_from_passphrase, public_key::PublicKey, public_key_hash::PublicKeyHash,
    secret_key::SecretKey,
};
use jstz_proto::context::account::Nonce;
use serde::{Deserialize, Serialize};

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub nonce: Nonce,
    pub alias: String,
    pub address: PublicKeyHash,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

impl Account {
    pub fn from_passphrase(passphrase: String, alias: String) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str()).unwrap();

        println!("Secret key: {}", sk.to_string());
        println!("Public key: {}", pk.to_string());

        let address = PublicKeyHash::try_from(&pk)?;
        let new_account = Account {
            nonce: Nonce::default(),
            alias,
            address,
            secret_key: sk,
            public_key: pk,
        };

        Ok(new_account)
    }
}
