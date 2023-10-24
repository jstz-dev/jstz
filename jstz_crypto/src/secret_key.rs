use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::SecretKeyBls;

use crate::error::Result;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum SecretKey {
    Bls(SecretKeyBls),
}

impl SecretKey {
    pub fn to_base58(&self) -> String {
        let SecretKey::Bls(pk) = self;
        pk.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let bls = SecretKeyBls::from_base58_check(data)?;

        Ok(SecretKey::Bls(bls))
    }
}

impl ToString for SecretKey {
    fn to_string(&self) -> String {
        self.to_base58()
    }
}
