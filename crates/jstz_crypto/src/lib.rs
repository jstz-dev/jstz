mod error;

pub use error::{Error, Result};
pub mod hash;
pub mod public_key;
pub mod public_key_hash;
pub mod secret_key;
pub mod signature;

use tezos_crypto_rs::hash::SeedEd25519;

use crate::{hash::Blake2b, public_key::PublicKey, secret_key::SecretKey};

pub fn keypair_from_passphrase(passphrase: &str) -> Result<(SecretKey, PublicKey)> {
    let ikm = Blake2b::from(passphrase.as_bytes()).as_array().to_vec();
    let seed = SeedEd25519(ikm);
    let (pk, sk) = seed.keypair()?;
    Ok((SecretKey::Ed25519(sk), PublicKey::Ed25519(pk)))
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
