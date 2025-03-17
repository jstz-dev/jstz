use std::fmt::Display;

use crate::{impl_bincode_for_hash, public_key::PublicKey, Error, Result};
use bincode::{Decode, Encode};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    hash::{Ed25519Signature, P256Signature, Secp256k1Signature},
    CryptoError, PublicKeySignatureVerifier,
};
use utoipa::ToSchema;

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Encode, Decode,
)]
#[serde(untagged)]
pub enum Signature {
    #[schema(
        title = "Ed25519 signature", 
        value_type = String,
        example = json!("edsigtpe2oRBMFdrrwf99ETNjmBaRzNDexDjhancfQdz5phrwyPPhRi9L7kzJD4cAW1fFcsyTJcTDPP8W4H168QPQdGPKe7jrZB")
    )]
    Ed25519(Ed25519),
    #[schema(
        title = "Secp256k1 signature", 
        value_type = String,
        example = json!("spsig1NajZUT4nSiWU7UiV98fmmsjApFFYwPHtiDiJfGMgGL6oP3U9SPEccTfhAPdnAcvJ6AUSQ8EBPxYNX4UeNNDLBxVg9qv5H")
    )]
    Secp256k1(Secp256k1),
    #[schema(
        title = "P256 signature", 
        value_type = String,
        example = json!("p2signEdtYeHXyWfCaGej9AFv7QraDsunRimyK47YGBQRNDEPXPQctwjPxbyFbTUtVLsACzG8QTrLAxddjjTRikF3nThwKL8nH")
    )]
    P256(P256),
}

// Newtype wrappesrs
#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ed25519(pub Ed25519Signature);

#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Secp256k1(pub Secp256k1Signature);

#[derive(Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct P256(pub P256Signature);

// Bincode implementation
impl_bincode_for_hash!(Ed25519, Ed25519Signature);
impl_bincode_for_hash!(Secp256k1, Secp256k1Signature);
impl_bincode_for_hash!(P256, P256Signature);

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
                verify_signature(&sig.0, &pk.0, message)
            }
            (Signature::Secp256k1(sig), PublicKey::Secp256k1(pk)) => {
                verify_signature(&sig.0, &pk.0, message)
            }
            (Signature::P256(sig), PublicKey::P256(pk)) => {
                verify_signature(&sig.0, &pk.0, message)
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
    use crate::{public_key::PublicKey, secret_key::SecretKey, signature::Signature};

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

    #[test]
    fn json_round_trip() {
        let sk = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        )
        .unwrap();
        let message = b"Hello, world!";
        let signature = sk.sign(message).unwrap();
        let json = serde_json::to_value(&signature).unwrap();
        assert_eq!(json, serde_json::json!("edsigtpe2oRBMFdrrwf99ETNjmBaRzNDexDjhancfQdz5phrwyPPhRi9L7kzJD4cAW1fFcsyTJcTDPP8W4H168QPQdGPKe7jrZB"));
        let decoded: Signature = serde_json::from_value(json).unwrap();
        assert_eq!(signature, decoded);
    }

    #[test]
    fn bin_round_trip() {
        let sk = SecretKey::from_base58(
            "edsk3AbxMYLgdY71xPEjWjXi5JCx6tSS8jhQ2mc1KczZ1JfPrTqSgM",
        )
        .unwrap();
        let message = b"Hello, world!";
        let signature = sk.sign(message).unwrap();
        let bin = bincode::encode_to_vec(&signature, bincode::config::legacy()).unwrap();
        let (decoded, _): (Signature, _) =
            bincode::decode_from_slice(bin.as_slice(), bincode::config::legacy())
                .unwrap();
        assert_eq!(signature, decoded);
    }
}
