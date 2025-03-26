mod error;

pub use error::{Error, Result};
pub mod hash;
pub mod public_key;
pub mod public_key_hash;
pub mod secret_key;
pub mod signature;
pub mod smart_function_hash;

use bip39::{Language, Mnemonic};
use tezos_crypto_rs::hash::SeedEd25519;

use crate::{public_key::PublicKey, secret_key::SecretKey};

pub fn keypair_from_passphrase(passphrase: &str) -> Result<(SecretKey, PublicKey)> {
    // FIXME: clarify terminology. The function input `passphrase` is actually equivalent to a
    // mnemonic in octez client.
    let m = Mnemonic::parse_in(Language::English, passphrase).unwrap();
    let seed = SeedEd25519::try_from(m.to_seed("")[0..32].to_vec()).unwrap();
    let (pk, sk) = seed.keypair()?;
    Ok((SecretKey::Ed25519(sk), PublicKey::Ed25519(pk.into())))
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

        impl Decode for $newtype {
            fn decode<D: bincode::de::Decoder>(
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
    use super::keypair_from_passphrase;
    use proptest::prelude::*;

    #[test]
    fn test_keypair_from_passphrase() {
        let s = "author crumble medal dose ribbon permit ankle sport final hood shadow vessel horn hawk enter zebra prefer devote captain during fly found despair business";
        let (_, pk) = keypair_from_passphrase(s).unwrap();
        assert_eq!(pk.hash(), "tz1ia78UBMgdmVf8b2vu5y8Rd148p9e2yn2h");
    }

    proptest! {
        #[test]
        fn test_keygen_verify(passphrase in any::<String>(), message in any::<Vec<u8>>()) {
            let (sk, pk) = keypair_from_passphrase(&passphrase).unwrap();
            let sig = sk.sign(&message).unwrap();
            assert!(sig.verify(&pk, &message).is_ok());
        }
    }
}
