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
