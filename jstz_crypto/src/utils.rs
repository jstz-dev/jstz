use anyhow::Result;
use tezos_crypto_rs::bls::keypair_from_ikm;

use crate::{hash::Blake2b, public_key::PublicKey, secret_key::SecretKey};

pub fn keypair_from_passphrase(passphrase: &str) -> Result<(SecretKey, PublicKey)> {
    let ikm = Blake2b::from(passphrase.as_bytes());

    let (sk, pk) = keypair_from_ikm(*ikm.as_array()).unwrap();

    Ok((SecretKey::Bls(sk), PublicKey::Bls(pk)))
}
