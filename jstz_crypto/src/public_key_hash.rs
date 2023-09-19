use std::fmt;

use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b::digest,
    hash::{ContractTz4Hash, HashTrait},
    PublicKeyWithHash,
};

use crate::{
    error::{Error, Result},
    public_key::PublicKey,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PublicKeyHash {
    Tz4(ContractTz4Hash),
}
impl PublicKeyHash {
    pub fn to_base58(&self) -> String {
        let PublicKeyHash::Tz4(tz4) = self;
        tz4.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let tz4 = ContractTz4Hash::from_base58_check(data)?;
        Ok(PublicKeyHash::Tz4(tz4))
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let tz4 = ContractTz4Hash::try_from_bytes(bytes)?;
        Ok(PublicKeyHash::Tz4(tz4))
    }

    pub fn as_bytes(&self) -> &[u8] {
        let PublicKeyHash::Tz4(tz4) = self;
        &tz4.0
    }
    pub fn digest(data: &[u8]) -> Result<Self> {
        let out_len = ContractTz4Hash::hash_size();
        let bytes = digest(data, out_len).expect("failed to create hash");
        Self::from_slice(&bytes)
    }
}

impl fmt::Display for PublicKeyHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl TryFrom<&PublicKey> for PublicKeyHash {
    type Error = Error;

    fn try_from(pk: &PublicKey) -> Result<Self> {
        let PublicKey::Bls(bls) = pk;
        let tz4 = bls.pk_hash()?;
        Ok(PublicKeyHash::Tz4(tz4))
    }
}
