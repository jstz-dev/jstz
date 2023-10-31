use anyhow::Result;
use jstz_crypto::{
    keypair_from_passphrase, public_key::PublicKey, public_key_hash::PublicKeyHash,
    secret_key::SecretKey,
};
use jstz_proto::context::account::Nonce;
use serde::{Deserialize, Serialize};

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Account {
    Owned {
        nonce: Nonce,
        alias: String,
        address: PublicKeyHash,
        secret_key: SecretKey,
        public_key: PublicKey,
        function_code: Option<String>,
    },
    Alias {
        alias: String,
        address: PublicKeyHash,
    },
}
impl Account {
    pub fn from_passphrase(
        passphrase: String,
        alias: String,
        function_code: Option<String>,
    ) -> Result<Self> {
        let (sk, pk) = keypair_from_passphrase(passphrase.as_str()).unwrap();

        println!("Secret key: {}", sk.to_string());
        println!("Public key: {}", pk.to_string());

        let address = PublicKeyHash::try_from(&pk)?;
        let owned_account = Account::Owned {
            nonce: Nonce::default(),
            alias,
            address,
            secret_key: sk,
            public_key: pk,
            function_code,
        };

        Ok(owned_account)
    }

    pub fn from_address(address: String, name: String) -> Result<Self> {
        let alias_account = Account::Alias {
            alias: name,
            address: PublicKeyHash::from_base58(address.as_str())?,
        };

        Ok(alias_account)
    }
}
