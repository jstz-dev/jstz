use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::PublicKeyEd25519;

use crate::error::Result;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum PublicKey {
    Ed25519(PublicKeyEd25519),
}

impl PublicKey {
    pub fn to_base58(&self) -> String {
        let PublicKey::Ed25519(pk) = self;
        pk.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let pk = PublicKeyEd25519::from_base58_check(data)?;

        Ok(PublicKey::Ed25519(pk))
    }
}

impl ToString for PublicKey {
    fn to_string(&self) -> String {
        self.to_base58()
    }
}
