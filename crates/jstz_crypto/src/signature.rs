use std::fmt::Display;

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

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_base58())
    }
}

#[cfg(test)]
mod test {
    use crate::{public_key::PublicKey, secret_key::SecretKey};

    #[test]
    fn verify_ed25519() {
        let sk = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        )
        .unwrap();
        let pk = PublicKey::from_base58(
            "edpkukK9ecWxib28zi52nvbXTdsYt8rYcvmt5bdH8KjipWXm8sH3Qi",
        )
        .unwrap();
        let message = b"Hello, world!";
        let signature = sk.sign(message).unwrap();

        assert!(signature.verify(&pk, message).is_ok());
    }

    #[test]
    fn base58() {
        let sk = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        )
        .unwrap();
        let message = b"Hello, world!";
        let signature = sk.sign(message).unwrap();
        assert_eq!(signature.to_string(), "edsigtpe2oRBMFdrrwf99ETNjmBaRzNDexDjhancfQdz5phrwyPPhRi9L7kzJD4cAW1fFcsyTJcTDPP8W4H168QPQdGPKe7jrZB");
    }
}
