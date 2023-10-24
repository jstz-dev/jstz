use anyhow::Result;
use jstz_crypto::{hash::Blake2b, public_key::PublicKey, public_key_hash::PublicKeyHash};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::bls::keypair_from_ikm;

fn create_address(pk: PublicKey) -> String {
    PublicKeyHash::try_from(&pk).unwrap().to_base58()
}

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    alias: String,
    address: String,
    secret_key: String,
    public_key: String,
}

impl Account {
    pub fn new(
        alias: String,
        address: String,
        secret_key: String,
        public_key: String,
    ) -> Self {
        Account {
            alias,
            address,
            secret_key,
            public_key,
        }
    }

    pub fn get_alias(&self) -> &String {
        &self.alias
    }

    pub fn get_address(&self) -> &String {
        &self.address
    }
    /*
    pub fn set_alias(&mut self, alias: String) {
        self.alias = alias;
    }

    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }
    */

    pub fn from_passphrase(passphrase: String, alias: String) -> Result<Self> {
        let ikm = Blake2b::from(passphrase.as_str().as_bytes());
        let (sk, pk) = keypair_from_ikm(*ikm.as_array()).unwrap();

        println!("Secret key: {}", sk);
        println!("Public key: {}", pk);

        let address = create_address(jstz_crypto::public_key::PublicKey::Bls(pk.clone()));
        let new_account =
            Account::new(alias, address.clone(), sk.to_string(), pk.to_string());

        Ok(new_account)
    }
}
