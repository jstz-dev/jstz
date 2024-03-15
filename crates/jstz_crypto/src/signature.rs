use serde::{Deserialize, Serialize};
use tezos_crypto_rs::PublicKeySignatureVerifier;

use crate::{public_key::PublicKey, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Signature {
    Ed25519(tezos_crypto_rs::hash::Signature),
}

impl Signature {
    pub fn to_base58(&self) -> String {
        match self {
            Signature::Ed25519(sig) => sig.to_base58_check(),
        }
    }
}

impl Signature {
    pub fn verify(&self, public_key: &PublicKey, message: &[u8]) -> Result<()> {
        match (self, public_key) {
            (Signature::Ed25519(sig), PublicKey::Ed25519(pk)) => {
                let result = pk.verify_signature(sig, message).unwrap();
                if result {
                    Ok(())
                } else {
                    Err(Error::InvalidSignature)
                }
            }
        }
    }
}
