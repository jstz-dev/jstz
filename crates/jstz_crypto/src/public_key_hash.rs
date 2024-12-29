use std::{fmt, str::FromStr};

use crate::hash::JstzHash;
use boa_gc::{empty_trace, Finalize, Trace};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b,
    hash::{ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashTrait},
    PublicKeyWithHash,
};
use utoipa::ToSchema;

use crate::{
    error::{Error, Result},
    public_key::PublicKey,
};

/// Tezos Address
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Finalize,
    ToSchema,
)]
#[serde(untagged)]
pub enum PublicKeyHash {
    #[schema(
        title = "Tz1",
        value_type = String,
        example = json!("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
    )]
    Tz1(ContractTz1Hash),
    #[schema(
        title = "Tz2",
        value_type = String,
        example =  json!("tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh")
    )]
    Tz2(ContractTz2Hash),
    #[schema(
        title = "Tz3",
        value_type = String,
        example = json!("tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C")
    )]
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
impl<'a> JstzHash<'a> for PublicKeyHash {
    fn to_base58(&self) -> String {
        match self {
            PublicKeyHash::Tz1(inner) => inner.to_b58check(),
            PublicKeyHash::Tz2(inner) => inner.to_b58check(),
            PublicKeyHash::Tz3(inner) => inner.to_b58check(),
        }
    }

    fn from_base58(data: &str) -> Result<Self> {
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

    fn as_bytes(&self) -> &[u8] {
        match self {
            PublicKeyHash::Tz1(inner) => inner.as_ref(),
            PublicKeyHash::Tz2(inner) => inner.as_ref(),
            PublicKeyHash::Tz3(inner) => inner.as_ref(),
        }
    }

    // `digest` does not guanrantee that the given data is a valid
    // Ed25519 address which semantically breaks the relationship between
    // Tz1 and the signature scheme. We currently depend on it to generate
    // new smart contract address but it should only be suitable in testing.
    // #[cfg(test)]
    fn digest(data: &[u8]) -> Result<Self> {
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

#[cfg(test)]
mod test {
    use crate::hash::JstzHash;
    use std::str::FromStr;

    use tezos_crypto_rs::hash::{
        ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashTrait,
    };

    use crate::public_key_hash::PublicKeyHash;

    const TZ1: &str = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU";
    const TZ2: &str = "tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh";
    const TZ3: &str = "tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C";

    #[test]
    fn from_str() {
        assert!(matches!(
            PublicKeyHash::from_str(TZ1).unwrap(),
            PublicKeyHash::Tz1(tz1) if tz1.to_b58check() == TZ1
        ));
        assert!(matches!(
            PublicKeyHash::from_str(TZ2).unwrap(),
            PublicKeyHash::Tz2(tz2) if tz2.to_b58check() == TZ2
        ));
        assert!(matches!(
            PublicKeyHash::from_str(TZ3).unwrap(),
            PublicKeyHash::Tz3(tz3) if tz3.to_b58check() == TZ3
        ));
        PublicKeyHash::from_str("invalid").expect_err("should fail");
        PublicKeyHash::from_str("tz1abc123").expect_err("should fail");
    }

    #[test]
    fn base58() {
        assert_eq!(PublicKeyHash::from_str(TZ1).unwrap().to_base58(), TZ1);
        assert_eq!(PublicKeyHash::from_str(TZ2).unwrap().to_base58(), TZ2);
        assert_eq!(PublicKeyHash::from_str(TZ3).unwrap().to_base58(), TZ3);
        assert_eq!(
            PublicKeyHash::from_base58(TZ1).unwrap(),
            PublicKeyHash::Tz1(ContractTz1Hash::from_base58_check(TZ1).unwrap())
        );
        assert_eq!(
            PublicKeyHash::from_base58(TZ2).unwrap(),
            PublicKeyHash::Tz2(ContractTz2Hash::from_base58_check(TZ2).unwrap())
        );
        assert_eq!(
            PublicKeyHash::from_base58(TZ3).unwrap(),
            PublicKeyHash::Tz3(ContractTz3Hash::from_base58_check(TZ3).unwrap())
        );
    }

    #[test]
    fn as_bytes() {
        assert_eq!(
            PublicKeyHash::Tz1(ContractTz1Hash::from_base58_check(TZ1).unwrap())
                .as_bytes(),
            ContractTz1Hash::from_base58_check(TZ1).unwrap().as_ref()
        );
        assert_eq!(
            PublicKeyHash::Tz2(ContractTz2Hash::from_base58_check(TZ2).unwrap())
                .as_bytes(),
            ContractTz2Hash::from_base58_check(TZ2).unwrap().as_ref()
        );
        assert_eq!(
            PublicKeyHash::Tz3(ContractTz3Hash::from_base58_check(TZ3).unwrap())
                .as_bytes(),
            ContractTz3Hash::from_base58_check(TZ3).unwrap().as_ref()
        );
    }

    #[test]
    fn digest_tz1() {
        let data = b"hello";
        let hash = PublicKeyHash::digest(data).unwrap();
        assert_eq!(
            hash.to_string(),
            "tz1cAnZVxXjLaDecxmBBgpeepZzZLFfisq1C".to_string()
        );
    }
}
