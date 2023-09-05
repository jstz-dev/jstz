use serde::{Deserialize, Serialize};
use tezos_crypto_rs::hash::BlsSignature;

use crate::{public_key::PublicKey, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Signature {
    Bls(BlsSignature),
}

impl Signature {
    pub fn verify(&self, public_key: &PublicKey, message: &[u8]) -> Result<()> {
        match (self, public_key) {
            (Signature::Bls(sig), PublicKey::Bls(pk)) => {
                let result = sig.aggregate_verify(&mut [(message, pk)].into_iter())?;
                if result {
                    Ok(())
                } else {
                    Err(Error::InvalidSignature)
                }
            }
        }
    }
}
