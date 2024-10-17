use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{CryptoError, PublicKeySignatureVerifier};

use crate::{public_key::PublicKey, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Signature {
    Ed25519(tezos_crypto_rs::hash::Ed25519Signature),
    Secp256k1(tezos_crypto_rs::hash::Secp256k1Signature),
    P256(tezos_crypto_rs::hash::P256Signature),
}

impl Signature {
    pub fn to_base58(&self) -> String {
        match self {
            Signature::Ed25519(sig) => sig.to_base58_check(),
            Signature::Secp256k1(sig) => sig.to_base58_check(),
            Signature::P256(sig) => sig.to_base58_check(),
        }
    }
}

impl Signature {
    pub fn verify(&self, public_key: &PublicKey, message: &[u8]) -> Result<()> {
        fn verify_signature<S, P>(sig: &S, pk: &P, message: &[u8]) -> Result<()>
        where
            P: PublicKeySignatureVerifier<Signature = S, Error = CryptoError>,
        {
            if pk.verify_signature(sig, message)? {
                Ok(())
            } else {
                Err(Error::InvalidSignature)
            }
        }

        match (self, public_key) {
            (Signature::Ed25519(sig), PublicKey::Ed25519(pk)) => {
                verify_signature(sig, pk, message)
            }
            (Signature::Secp256k1(sig), PublicKey::Secp256k1(pk)) => {
                verify_signature(sig, pk, message)
            }
            (Signature::P256(sig), PublicKey::P256(pk)) => {
                verify_signature(sig, pk, message)
            }
            _ => Err(Error::InvalidSignature),
        }
    }
}
