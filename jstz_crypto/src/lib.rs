mod error;

pub use error::{Error, Result};
pub mod hash;
pub mod public_key;
pub mod public_key_hash;
pub mod secret_key;
pub mod signature;

use tezos_crypto_rs::bls;

use crate::{hash::Blake2b, public_key::PublicKey, secret_key::SecretKey};

pub fn keypair_from_passphrase(passphrase: &str) -> Result<(SecretKey, PublicKey)> {
    let ikm = Blake2b::from(passphrase.as_bytes());

    let (sk, pk) = bls::keypair_from_ikm(*ikm.as_array())?;

    Ok((SecretKey::Bls(sk), PublicKey::Bls(pk)))
}
