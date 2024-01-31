use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::SecretKeyEd25519;

use crate::{error::Result, signature::Signature};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum SecretKey {
    Ed25519(SecretKeyEd25519),
}

impl SecretKey {
    pub fn to_base58(&self) -> String {
        let SecretKey::Ed25519(pk) = self;
        pk.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let sk = SecretKeyEd25519::from_base58_check(data)?;

        Ok(SecretKey::Ed25519(sk))
    }

    pub fn sign(&self, message: impl AsRef<[u8]>) -> Result<Signature> {
        let SecretKey::Ed25519(sk) = self;
        Ok(Signature::Ed25519(sk.sign(message)?))
    }
}

impl ToString for SecretKey {
    fn to_string(&self) -> String {
        self.to_base58()
    }
}
