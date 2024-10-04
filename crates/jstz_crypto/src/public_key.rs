use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1};

use crate::{error::Result, Error};

// FIXME: https://linear.app/tezos/issue/JSTZ-169/support-bls-in-risc-v
// Add BLS support
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum PublicKey {
    Ed25519(PublicKeyEd25519),
    Secp256k1(PublicKeySecp256k1),
    P256(PublicKeyP256),
}

impl PublicKey {
    pub fn to_base58(&self) -> String {
        match self {
            PublicKey::Ed25519(pk) => pk.to_base58_check(),
            PublicKey::Secp256k1(pk) => pk.to_base58_check(),
            PublicKey::P256(pk) => pk.to_base58_check(),
        }
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        match &data[..4] {
            "edpk" => {
                let pk = PublicKeyEd25519::from_base58_check(data)?;
                Ok(PublicKey::Ed25519(pk))
            }
            "sppk" => {
                let pk = PublicKeySecp256k1::from_base58_check(data)?;
                Ok(PublicKey::Secp256k1(pk))
            }
            "p2pk" => {
                let pk = PublicKeyP256::from_base58_check(data)?;
                Ok(PublicKey::P256(pk))
            }
            _ => Err(Error::InvalidPublicKey),
        }
    }
}

impl ToString for PublicKey {
    fn to_string(&self) -> String {
        self.to_base58()
    }
}
