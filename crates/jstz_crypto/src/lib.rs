mod error;

pub use error::{Error, Result};
pub mod hash;
pub mod public_key;
pub mod public_key_hash;
pub mod secret_key;
pub mod signature;
pub mod smart_function_hash;
pub mod verifier;

pub use tezos_crypto_rs::hash::HashTrait;

use crate::{public_key::PublicKey, secret_key::SecretKey};
use bip39::{Language, Mnemonic};
use tezos_crypto_rs::{hash::SeedEd25519, CryptoError};

pub fn keypair_from_mnemonic(
    mnemonic: &str,
    passphrase: &str,
) -> Result<(PublicKey, SecretKey)> {
    let m = Mnemonic::parse_in(Language::English, mnemonic).map_err(|e| {
        CryptoError::InvalidKey {
            reason: format!("failed to parse mnemonic: {e}"),
        }
    })?;
    let seed = SeedEd25519::try_from(m.to_seed(passphrase)[0..32].to_vec())?;
    let (pk, sk) = seed.keypair()?;
    Ok((PublicKey::Ed25519(pk.into()), SecretKey::Ed25519(sk)))
}

pub fn keypair_from_secret_key(secret_key_str: &str) -> Result<(PublicKey, SecretKey)> {
    let seed = SeedEd25519::from_base58_check(secret_key_str)?;
    let (pk, sk) = seed.keypair()?;
    Ok((PublicKey::Ed25519(pk.into()), SecretKey::Ed25519(sk)))
}

#[macro_export]
macro_rules! impl_bincode_for_hash {
    ($newtype:ident, $hash:ident) => {
        impl Encode for $newtype {
            fn encode<E: bincode::enc::Encoder>(
                &self,
                encoder: &mut E,
            ) -> std::result::Result<(), bincode::error::EncodeError> {
                Encode::encode(&self.0.as_ref(), encoder)
            }
        }

        impl<C> Decode<C> for $newtype {
            fn decode<D: bincode::de::Decoder<Context = C>>(
                decoder: &mut D,
            ) -> std::result::Result<Self, bincode::error::DecodeError> {
                let raw_hash: Vec<u8> = Decode::decode(decoder)?;
                Ok($hash::try_from(raw_hash)
                    .map_err(|e| bincode::error::DecodeError::OtherString(e.to_string()))?
                    .into())
            }
        }

        bincode::impl_borrow_decode!($newtype);
    };
}

#[cfg(test)]
mod tests {
    use super::keypair_from_mnemonic;
    use proptest::prelude::*;

    #[test]
    fn keypair_from_mnemonic_should_align_with_octez_client() {
        let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
        let (pk, _) = keypair_from_mnemonic(mnemonic, "").unwrap();
        // This address is acquired from octez-client with the mnemonic above and an empty passphrase:
        // echo $'author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business\n' | octez-client import keys from mnemonic test --force
        assert_eq!(pk.hash(), "tz1ia78UBMgdmVf8b2vu5y8Rd148p9e2yn2h");

        let (pk, _) = keypair_from_mnemonic(mnemonic, "foobar").unwrap();
        // This address is acquired from octez-client with the mnemonic above and passphrase 'foobar':
        // echo $'author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business\nfoobar\n' | octez-client import keys from mnemonic test --force
        assert_eq!(pk.hash(), "tz1W8rEphWEjMcD1HsxEhsBFocfMeGsW7Qxg");
    }

    #[test]
    fn keypair_from_mnemonic_failed() {
        assert_eq!(keypair_from_mnemonic("a", "").unwrap_err().to_string(), "Invalid crypto key, reason: failed to parse mnemonic: mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24");
    }

    proptest! {
        #[test]
        fn test_keygen_verify(passphrase in any::<String>(), message in any::<Vec<u8>>()) {
            let mnemonic = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
            let (pk, sk) = keypair_from_mnemonic(mnemonic, &passphrase).unwrap();
            let sig = sk.sign(&message).unwrap();
            assert!(sig.verify(&pk, &message).is_ok());
        }
    }

    #[test]
    fn keypair_from_secret_key() {
        let (pk, sk) = super::keypair_from_secret_key(
            "edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf",
        )
        .unwrap();

        assert_eq!(
            sk.to_string(),
            "edsk3a3gq6ocr51rGDqqSb8sxxV46v77GZYmhyKyjqWjckhVTJXYCf"
        );
        assert_eq!(pk.hash(), "tz1ficxJFv7MUtsCimF8bmT9SYPDok52ySg6");

        assert_eq!(
            super::keypair_from_secret_key("edskaaa")
                .unwrap_err()
                .to_string(),
            "invalid checksum"
        );
    }
}
