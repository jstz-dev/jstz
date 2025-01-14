use std::{fmt, str::FromStr};

use crate::hash::Hash;
use crate::impl_bincode_for_hash;
use crate::{
    error::{Error, Result},
    public_key::PublicKey,
};
use bincode::{Decode, Encode};
use boa_gc::{empty_trace, Finalize, Trace};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use tezos_crypto_rs::{
    blake2b,
    hash::{ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashTrait},
    PublicKeyWithHash,
};
use utoipa::ToSchema;
/// Tezos Address
#[derive(
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
    Encode,
    Decode,
)]
#[serde(untagged)]
pub enum PublicKeyHash {
    #[schema(
        title = "Tz1",
        value_type = String,
        example = json!("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
    )]
    Tz1(Tz1),
    #[schema(
        title = "Tz2",
        value_type = String,
        example =  json!("tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh")
    )]
    Tz2(Tz2),
    #[schema(
        title = "Tz3",
        value_type = String,
        example = json!("tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C")
    )]
    Tz3(Tz3),
}

// Newtype wrappers
#[derive(
    Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Finalize,
)]
pub struct Tz1(ContractTz1Hash);

#[derive(
    Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Finalize,
)]
pub struct Tz2(ContractTz2Hash);

#[derive(
    Deref, From, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Finalize,
)]
pub struct Tz3(ContractTz3Hash);

// Bincode implementation
impl_bincode_for_hash!(Tz1, ContractTz1Hash);
impl_bincode_for_hash!(Tz2, ContractTz2Hash);
impl_bincode_for_hash!(Tz3, ContractTz3Hash);

unsafe impl Trace for PublicKeyHash {
    empty_trace!();
}

impl FromStr for PublicKeyHash {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        PublicKeyHash::from_base58(s)
    }
}
impl<'a> Hash<'a> for PublicKeyHash {
    fn to_base58(&self) -> String {
        match self {
            PublicKeyHash::Tz1(inner) => inner.to_b58check(),
            PublicKeyHash::Tz2(inner) => inner.to_b58check(),
            PublicKeyHash::Tz3(inner) => inner.to_b58check(),
        }
    }

    fn from_base58(data: &str) -> Result<Self> {
        match &data[..3] {
            "tz1" => Ok(PublicKeyHash::Tz1(
                ContractTz1Hash::from_base58_check(data)?.into(),
            )),
            "tz2" => Ok(PublicKeyHash::Tz2(
                ContractTz2Hash::from_base58_check(data)?.into(),
            )),
            "tz3" => Ok(PublicKeyHash::Tz3(
                ContractTz3Hash::from_base58_check(data)?.into(),
            )),
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
        let bytes = blake2b::digest(data, out_len)?;
        Ok(PublicKeyHash::Tz1(
            ContractTz1Hash::try_from_bytes(&bytes)?.into(),
        ))
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
            PublicKey::Ed25519(pk) => PublicKeyHash::Tz1(pk.pk_hash().into()),
            PublicKey::Secp256k1(pk) => PublicKeyHash::Tz2(pk.pk_hash().into()),
            PublicKey::P256(pk) => PublicKeyHash::Tz3(pk.pk_hash().into()),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::hash::Hash;
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
            PublicKeyHash::Tz1(ContractTz1Hash::from_base58_check(TZ1).unwrap().into())
        );
        assert_eq!(
            PublicKeyHash::from_base58(TZ2).unwrap(),
            PublicKeyHash::Tz2(ContractTz2Hash::from_base58_check(TZ2).unwrap().into())
        );
        assert_eq!(
            PublicKeyHash::from_base58(TZ3).unwrap(),
            PublicKeyHash::Tz3(ContractTz3Hash::from_base58_check(TZ3).unwrap().into())
        );
    }

    #[test]
    fn as_bytes() {
        assert_eq!(
            PublicKeyHash::Tz1(ContractTz1Hash::from_base58_check(TZ1).unwrap().into())
                .as_bytes(),
            ContractTz1Hash::from_base58_check(TZ1).unwrap().as_ref()
        );
        assert_eq!(
            PublicKeyHash::Tz2(ContractTz2Hash::from_base58_check(TZ2).unwrap().into())
                .as_bytes(),
            ContractTz2Hash::from_base58_check(TZ2).unwrap().as_ref()
        );
        assert_eq!(
            PublicKeyHash::Tz3(ContractTz3Hash::from_base58_check(TZ3).unwrap().into())
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

    #[test]
    fn json_round_trip() {
        let pkh = PublicKeyHash::from_base58(TZ1).unwrap();
        let json = serde_json::to_value(&pkh).unwrap();
        assert_eq!(
            json,
            serde_json::json!("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
        );
        let decoded: PublicKeyHash = serde_json::from_value(json).unwrap();
        assert_eq!(pkh, decoded);
    }

    #[test]
    fn bin_round_trip() {
        let pkh = PublicKeyHash::from_base58(TZ1).unwrap();
        let bin = bincode::encode_to_vec(&pkh, bincode::config::legacy()).unwrap();
        let (decoded, _): (PublicKeyHash, usize) =
            bincode::decode_from_slice(bin.as_ref(), bincode::config::legacy()).unwrap();
        assert_eq!(pkh, decoded);
    }
}
