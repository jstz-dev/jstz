use std::{fmt::Display, str::FromStr};

use crate::{public_key::PublicKey, Error, Result};
use jstz_macro::SerdeCrypto;
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::{Ed25519Signature, P256Signature, Secp256k1Signature};
use tezos_crypto_rs::{CryptoError, PublicKeySignatureVerifier};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, SerdeCrypto)]
pub enum Signature {
    Ed25519(Ed25519Signature),
    Secp256k1(Secp256k1Signature),
    P256(P256Signature),
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

impl FromStr for Signature {
    type Err = Error;

    fn from_str(data: &str) -> Result<Signature> {
        match &data[..5] {
            "edsig" => Ok(Signature::Ed25519(Ed25519Signature::from_base58_check(
                data,
            )?)),
            "spsig" => Ok(Signature::Secp256k1(Secp256k1Signature::from_base58_check(
                data,
            )?)),
            "p2sig" => Ok(Signature::P256(P256Signature::from_base58_check(data)?)),
            _ => Err(Error::InvalidSignature),
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

mod openapi {
    use serde_json::json;
    use utoipa::{
        openapi::{schema::Schema, ObjectBuilder, OneOfBuilder, RefOr, Type},
        PartialSchema, ToSchema,
    };

    use super::Signature;

    impl ToSchema for Signature {
        fn name() -> std::borrow::Cow<'static, str> {
            std::borrow::Cow::Borrowed("Signature")
        }
    }

    impl PartialSchema for Signature {
        fn schema() -> RefOr<Schema> {
            let one_of = OneOfBuilder::new()
                .item(
                    ObjectBuilder::new()
                        .title(Some("Ed25519"))
                        .schema_type(Type::String)
                        .build(),
                )
                .item(
                    ObjectBuilder::new()
                        .title(Some("Secp256k1"))
                        .schema_type(Type::String)
                        .build(),
                )
                .item(
                    ObjectBuilder::new()
                        .title(Some("P256"))
                        .schema_type(Type::String)
                        .build(),
                )
                .examples([
                    json!("edsigtpe2oRBMFdrrwf99ETNjmBaRzNDexDjhancfQdz5phrwyPPhRi9L7kzJD4cAW1fFcsyTJcTDPP8W4H168QPQdGPKe7jrZB"),
                    json!("spsig1NajZUT4nSiWU7UiV98fmmsjApFFYwPHtiDiJfGMgGL6oP3U9SPEccTfhAPdnAcvJ6AUSQ8EBPxYNX4UeNNDLBxVg9qv5H"),
                    json!("p2signEdtYeHXyWfCaGej9AFv7QraDsunRimyK47YGBQRNDEPXPQctwjPxbyFbTUtVLsACzG8QTrLAxddjjTRikF3nThwKL8nH"),
                ])
                .build();
            RefOr::T(Schema::OneOf(one_of))
        }
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
        let json = serde_json::to_string(&signature).unwrap();
        let decoded: Signature = serde_json::from_str(&json).unwrap();
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
        let bin = bincode::serialize(&signature).unwrap();
        let decoded = bincode::deserialize::<Signature>(bin.as_slice()).unwrap();
        assert_eq!(signature, decoded);
    }
}
