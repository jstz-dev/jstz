use std::fmt;
use tezos_crypto_rs::hash::ContractTz4Hash;

use tezos_crypto_rs::PublicKeyWithHash;

use crate::{
    error::{Error, Result},
    public_key::PublicKey,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
