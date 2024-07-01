use std::{fmt, str::FromStr};

use boa_gc::{empty_trace, Finalize, Trace};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b::digest,
    hash::{ContractTz1Hash, HashTrait},
    PublicKeyWithHash,
};

use crate::{
    error::{Error, Result},
    public_key::PublicKey,
};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Finalize,
)]
pub enum PublicKeyHash {
    Tz1(ContractTz1Hash),
}

unsafe impl Trace for PublicKeyHash {
    empty_trace!();
}

impl FromStr for PublicKeyHash {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        PublicKeyHash::from_base58(s)
    }
}

impl PublicKeyHash {
    pub fn to_base58(&self) -> String {
        let PublicKeyHash::Tz1(tz1) = self;
        tz1.to_base58_check()
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        let tz1 = ContractTz1Hash::from_base58_check(data)?;
        Ok(PublicKeyHash::Tz1(tz1))
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let tz1 = ContractTz1Hash::try_from_bytes(bytes)?;
        Ok(PublicKeyHash::Tz1(tz1))
    }

    pub fn as_bytes(&self) -> &[u8] {
        let PublicKeyHash::Tz1(tz1) = self;
        tz1.as_ref()
    }
    pub fn digest(data: &[u8]) -> Result<Self> {
        let out_len = ContractTz1Hash::hash_size();
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
        let PublicKey::Ed25519(key) = pk;
        let tz1 = key.pk_hash();
        Ok(PublicKeyHash::Tz1(tz1))
    }
}
