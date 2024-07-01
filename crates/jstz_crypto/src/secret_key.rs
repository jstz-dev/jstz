use std::fmt::{self, Debug};

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::SecretKeyEd25519;
use tezos_crypto_rs::hash::SeedEd25519;

use crate::{error::Result, signature::Signature};

// FIXME: workaround via `SeedEd25519` will be unnecessary in the next tezos_crypto_rs release
//        (will be included in next SDK release)

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(from = "SecretKeySerde", into = "SecretKeySerde")]
pub enum SecretKey {
    Ed25519(SecretKeyEd25519),
}

impl Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretKey").field(&self.to_base58()).finish()
    }
}

impl SecretKey {
    pub fn to_base58(&self) -> String {
        let Self::Ed25519(sk) = self;
        sk.to_base58_check()
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
enum SecretKeySerde {
    Ed25519(SeedEd25519),
}

impl From<SecretKey> for SecretKeySerde {
    fn from(s: SecretKey) -> Self {
        let SecretKey::Ed25519(sk) = s;
        let sk: Vec<u8> = sk.into();
        Self::Ed25519(SeedEd25519::try_from(sk).unwrap())
    }
}

impl From<SecretKeySerde> for SecretKey {
    fn from(s: SecretKeySerde) -> Self {
        let SecretKeySerde::Ed25519(sk) = s;
        let sk: Vec<u8> = sk.into();
        Self::Ed25519(SecretKeyEd25519::try_from(sk).unwrap())
    }
}
