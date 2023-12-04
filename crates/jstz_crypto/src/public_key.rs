use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::PublicKeyBls;

use crate::error::Result;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum PublicKey {
    Bls(PublicKeyBls),
}

impl PublicKey {
    pub fn to_base58(&self) -> String {
        let PublicKey::Bls(pk) = self;
        pk.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let bls = PublicKeyBls::from_base58_check(data)?;

        Ok(PublicKey::Bls(bls))
    }
}

impl ToString for PublicKey {
    fn to_string(&self) -> String {
        self.to_base58()
    }
}
