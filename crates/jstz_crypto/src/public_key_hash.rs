use std::{fmt, str::FromStr};

use boa_gc::{empty_trace, Finalize, Trace};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b,
    hash::{ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashTrait},
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
    Tz2(ContractTz2Hash),
    Tz3(ContractTz3Hash),
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
        match self {
            PublicKeyHash::Tz1(inner) => inner.to_b58check(),
            PublicKeyHash::Tz2(inner) => inner.to_b58check(),
            PublicKeyHash::Tz3(inner) => inner.to_b58check(),
        }
    }

    pub fn from_base58(data: &str) -> Result<Self> {
        match &data[..3] {
            "tz1" => Ok(PublicKeyHash::Tz1(ContractTz1Hash::from_base58_check(
                data,
            )?)),
            "tz2" => Ok(PublicKeyHash::Tz2(ContractTz2Hash::from_base58_check(
                data,
            )?)),
            "tz3" => Ok(PublicKeyHash::Tz3(ContractTz3Hash::from_base58_check(
                data,
            )?)),
            _ => Err(Error::InvalidPublicKeyHash),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            PublicKeyHash::Tz1(inner) => inner.as_ref(),
            PublicKeyHash::Tz2(inner) => inner.as_ref(),
            PublicKeyHash::Tz3(inner) => inner.as_ref(),
        }
    }

    // `digest_tz1` does not guanrantee that the given data is a valid
    // Ed25519 address which semantically breaks the relationship between
    // Tz1 and the signature scheme. We currently depend on it to generate
    // new smart contract address but it should only be suitable in testing.
    // #[cfg(test)]
    pub fn digest_tz1(data: &[u8]) -> Result<Self> {
        let out_len = ContractTz1Hash::hash_size();
        let bytes = blake2b::digest(data, out_len).expect("failed to create hash");
        Ok(PublicKeyHash::Tz1(ContractTz1Hash::try_from_bytes(&bytes)?))
    }
}

impl fmt::Display for PublicKeyHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl From<&PublicKey> for PublicKeyHash {
    fn from(pk: &PublicKey) -> Self {
        match pk {
            PublicKey::Ed25519(pk) => PublicKeyHash::Tz1(pk.pk_hash()),
            PublicKey::Secp256k1(pk) => PublicKeyHash::Tz2(pk.pk_hash()),
            PublicKey::P256(pk) => PublicKeyHash::Tz3(pk.pk_hash()),
        }
    }
}
