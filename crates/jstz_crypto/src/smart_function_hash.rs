use crate::{
    error::{Error, Result},
    hash::Hash,
    impl_bincode_for_hash,
};
use bincode::{Decode, Encode};
use boa_gc::{empty_trace, Finalize, Trace};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use tezos_crypto_rs::{
    blake2b,
    hash::{ContractKt1Hash, HashTrait},
};
use utoipa::{schema, ToSchema};

#[derive(
    Deref,
    From,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Finalize,
    ToSchema,
)]
#[schema(
    title = "KT1",
    value_type = String,
    example = json!("KT1RycYvM4EVs6BAXWEsGXaAaRqiMP53KT4w")
)]
pub struct Kt1Hash(pub ContractKt1Hash);

impl_bincode_for_hash!(Kt1Hash, ContractKt1Hash);

#[derive(
    Deref,
    From,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Finalize,
    Encode,
    Decode,
    ToSchema,
)]
pub struct SmartFunctionHash(pub Kt1Hash);

unsafe impl Trace for SmartFunctionHash {
    empty_trace!();
}

impl From<SmartFunctionHash> for ContractKt1Hash {
    fn from(value: SmartFunctionHash) -> ContractKt1Hash {
        value.0 .0
    }
}

impl From<ContractKt1Hash> for SmartFunctionHash {
    fn from(value: ContractKt1Hash) -> Self {
        Kt1Hash(value).into()
    }
}

impl FromStr for SmartFunctionHash {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        SmartFunctionHash::from_base58(s)
    }
}

impl<'a> Hash<'a> for SmartFunctionHash {
    fn to_base58(&self) -> String {
        match self {
            SmartFunctionHash(inner) => inner.to_b58check(),
        }
    }

    fn from_base58(data: &str) -> Result<Self> {
        if data.len() < 3 {
            return Err(Error::InvalidSmartFunctionHash);
        }
        match &data[..3] {
            "KT1" => Ok(SmartFunctionHash(
                ContractKt1Hash::from_base58_check(data)?.into(),
            )),
            _ => Err(Error::InvalidSmartFunctionHash),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            SmartFunctionHash(inner) => inner.as_ref(),
        }
    }

    // Generate a new contract address by hashing the input data.
    // This is the standard way to generate KT1 addresses for smart contracts.
    fn digest(data: &[u8]) -> Result<Self> {
        let out_len = ContractKt1Hash::hash_size();
        let bytes = blake2b::digest(data, out_len)?;
        Ok(SmartFunctionHash(
            ContractKt1Hash::try_from_bytes(&bytes)?.into(),
        ))
    }
}

impl fmt::Display for SmartFunctionHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    const KT1_VALID: &str = "KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton";
    const KT1_INVALID: &str = "KT1invalidaddress";
    const UNKNOWN_PREFIX: &str = "KT2RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton";

    #[test]
    fn from_str_valid() {
        let hash = SmartFunctionHash::from_str(KT1_VALID).unwrap();
        match hash {
            SmartFunctionHash(inner) => {
                assert_eq!(inner.to_b58check(), KT1_VALID);
            }
        }
    }

    #[test]
    fn from_str_invalid() {
        // Test with an invalid KT1 address
        assert!(SmartFunctionHash::from_str(KT1_INVALID).is_err());

        // Test with an unknown prefix
        assert!(SmartFunctionHash::from_str(UNKNOWN_PREFIX).is_err());

        // Test with completely invalid format
        assert!(SmartFunctionHash::from_str("invalid").is_err());

        assert!(SmartFunctionHash::from_str("a").is_err());
    }

    #[test]
    fn to_base58() {
        let hash = SmartFunctionHash::from_str(KT1_VALID).unwrap();
        assert_eq!(hash.to_base58(), KT1_VALID);
    }

    #[test]
    fn as_bytes() {
        let hash = SmartFunctionHash::from_str(KT1_VALID).unwrap();
        // Assuming KT1 hashes are 20 bytes; adjust if different
        assert_eq!(hash.as_bytes().len(), 20);
    }

    #[test]
    fn display_trait() {
        let hash = SmartFunctionHash::from_str(KT1_VALID).unwrap();
        assert_eq!(hash.to_string(), KT1_VALID);
    }

    #[test]
    fn digest() {
        let hash = SmartFunctionHash::digest(b"hello").unwrap();
        assert_eq!(hash.to_base58(), "KT1R7XS9SPbsf9ri9fUwjBLff8LB4oYCc4ao");
    }
}
