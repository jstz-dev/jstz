use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crate::error::{Error, Result};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_crypto::hash::Hash;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
// TODO: rename to Address
// https://linear.app/tezos/issue/JSTZ-253/remove-old-accountrs
#[schema(as = Address)]
#[schema(description = "Tezos Address")]
pub enum NewAddress {
    User(PublicKeyHash),
    SmartFunction(SmartFunctionHash),
}

unsafe impl Trace for NewAddress {
    empty_trace!();
}

impl Display for NewAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(hash) => hash.fmt(f),
            Self::SmartFunction(hash) => hash.fmt(f),
        }
    }
}

impl FromStr for NewAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_base58(s)
    }
}

impl NewAddress {
    pub fn from_base58(data: &str) -> Result<Self> {
        if data.len() < 3 {
            return Err(Error::InvalidAddress);
        }
        match &data[..3] {
            "KT1" => Ok(NewAddress::SmartFunction(SmartFunctionHash::from_base58(
                data,
            )?)),
            "tz1" | "tz2" | "tz3" => {
                Ok(NewAddress::User(PublicKeyHash::from_base58(data)?))
            }
            _ => Err(Error::InvalidAddress),
        }
    }

    pub fn to_base58(&self) -> String {
        match self {
            Self::User(hash) => hash.to_base58(),
            Self::SmartFunction(hash) => hash.to_base58(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            NewAddress::User(hash) => hash.as_bytes(),
            NewAddress::SmartFunction(hash) => hash.as_bytes(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_crypto::hash::Hash;

    const TZ1: &str = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU";
    const TZ2: &str = "tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh";
    const TZ3: &str = "tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C";
    const KT1: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";

    #[test]
    fn test_from_str_all_types() {
        // Test tz1 (Ed25519)
        let tz1_addr = NewAddress::from_str(TZ1).unwrap();
        assert!(matches!(
            tz1_addr,
            NewAddress::User(pkh) if pkh.to_base58() == TZ1
        ));

        // Test tz2 (Secp256k1)
        let tz2_addr = NewAddress::from_str(TZ2).unwrap();
        assert!(matches!(
            tz2_addr,
            NewAddress::User(pkh) if pkh.to_base58() == TZ2
        ));

        // Test tz3 (P256)
        let tz3_addr = NewAddress::from_str(TZ3).unwrap();
        assert!(matches!(
            tz3_addr,
            NewAddress::User(pkh) if pkh.to_base58() == TZ3
        ));

        // Test KT1 (Smart Function)
        let kt1_addr = NewAddress::from_str(KT1).unwrap();
        assert!(matches!(
            kt1_addr,
            NewAddress::SmartFunction(hash) if hash.to_base58() == KT1
        ));
    }

    #[test]
    fn test_invalid_addresses() {
        // Test invalid prefix
        assert!(matches!(
            NewAddress::from_str("tx1abc123"),
            Err(Error::InvalidAddress)
        ));

        // Test invalid tz1
        assert!(NewAddress::from_str("tz1invalid").is_err());

        // Test invalid tz2
        assert!(NewAddress::from_str("tz2invalid").is_err());

        // Test invalid tz3
        assert!(NewAddress::from_str("tz3invalid").is_err());

        // Test invalid KT1
        assert!(NewAddress::from_str("KT1invalid").is_err());
    }

    #[test]
    fn test_display() {
        // Test tz1 display
        let tz1_addr = NewAddress::from_str(TZ1).unwrap();
        assert_eq!(tz1_addr.to_string(), TZ1);

        // Test tz2 display
        let tz2_addr = NewAddress::from_str(TZ2).unwrap();
        assert_eq!(tz2_addr.to_string(), TZ2);

        // Test tz3 display
        let tz3_addr = NewAddress::from_str(TZ3).unwrap();
        assert_eq!(tz3_addr.to_string(), TZ3);

        // Test KT1 display
        let kt1_addr = NewAddress::from_str(KT1).unwrap();
        assert_eq!(kt1_addr.to_string(), KT1);
    }

    #[test]
    fn test_from_base58() {
        // Test valid addresses
        let tz1_addr = NewAddress::from_base58(TZ1).unwrap();
        assert!(matches!(
            tz1_addr,
            NewAddress::User(pkh) if pkh.to_base58() == TZ1
        ));

        let kt1_addr = NewAddress::from_base58(KT1).unwrap();
        assert!(matches!(
            kt1_addr,
            NewAddress::SmartFunction(hash) if hash.to_base58() == KT1
        ));

        // Test invalid prefixes
        assert!(matches!(
            NewAddress::from_base58("tx1abc123"),
            Err(Error::InvalidAddress)
        ));

        // Test invalid formats
        assert!(NewAddress::from_base58("tz1invalid").is_err());
        assert!(NewAddress::from_base58("KT1invalid").is_err());

        // Test empty string
        assert!(matches!(
            NewAddress::from_base58(""),
            Err(Error::InvalidAddress)
        ));

        // Test string too short for prefix check
        assert!(matches!(
            NewAddress::from_base58("tz"),
            Err(Error::InvalidAddress)
        ));
    }

    #[test]
    fn test_from_base58_error() {
        let invalid_checksum = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjV"; // Changed last char
        let result = NewAddress::from_base58(invalid_checksum);
        assert!(result.is_err());

        let invalid_kt1 = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU6"; // Changed last char
        let result = NewAddress::from_base58(invalid_kt1);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_base58() {
        // Test User addresses
        let tz1_addr = NewAddress::from_str(TZ1).unwrap();
        assert_eq!(tz1_addr.to_base58(), TZ1);

        let tz2_addr = NewAddress::from_str(TZ2).unwrap();
        assert_eq!(tz2_addr.to_base58(), TZ2);

        let tz3_addr = NewAddress::from_str(TZ3).unwrap();
        assert_eq!(tz3_addr.to_base58(), TZ3);

        // Test SmartFunction address
        let kt1_addr = NewAddress::from_str(KT1).unwrap();
        assert_eq!(kt1_addr.to_base58(), KT1);

        // Test roundtrip
        let addr = NewAddress::from_base58(&kt1_addr.to_base58()).unwrap();
        assert_eq!(addr, kt1_addr);
    }

    #[test]
    fn test_as_bytes() {
        // Test User address bytes
        let tz1_addr = NewAddress::from_str(TZ1).unwrap();
        let tz1_bytes = tz1_addr.as_bytes();
        assert!(!tz1_bytes.is_empty());

        // Test SmartFunction address bytes
        let kt1_addr = NewAddress::from_str(KT1).unwrap();
        let kt1_bytes = kt1_addr.as_bytes();
        assert!(!kt1_bytes.is_empty());

        // Verify that converting back to base58 works
        assert_eq!(tz1_addr.to_base58(), TZ1);
        assert_eq!(kt1_addr.to_base58(), KT1);
    }
}
