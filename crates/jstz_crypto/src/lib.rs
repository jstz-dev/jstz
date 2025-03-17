mod error;

pub use error::{Error, Result};
pub mod hash;
pub mod public_key;
pub mod public_key_hash;
pub mod secret_key;
pub mod signature;
pub mod smart_function_hash;

use tezos_crypto_rs::hash::SeedEd25519;

use crate::{hash::Blake2b, public_key::PublicKey, secret_key::SecretKey};

pub fn keypair_from_passphrase(passphrase: &str) -> Result<(SecretKey, PublicKey)> {
    let ikm = Blake2b::from(passphrase.as_bytes()).as_array().to_vec();
    let seed = SeedEd25519::try_from(ikm)?;
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

    proptest! {
        #[test]
        fn test_keygen_verify(passphrase in any::<String>(), message in any::<Vec<u8>>()) {
            let (sk, pk) = keypair_from_passphrase(&passphrase).unwrap();
            let sig = sk.sign(&message).unwrap();
            assert!(sig.verify(&pk, &message).is_ok());
        }
    }
}
