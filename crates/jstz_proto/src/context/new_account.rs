use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crate::error::{Error, Result};
use boa_gc::{empty_trace, Finalize, Trace};
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
pub enum NewAddress {
    User(PublicKeyHash),
    SF(SmartFunctionHash),
}

unsafe impl Trace for NewAddress {
    empty_trace!();
}

impl Display for NewAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(hash) => hash.fmt(f),
            Self::SF(hash) => hash.fmt(f),
        }
    }
}

pub const JSTZ_ADDRESS_PREFIXES: [&str; 4] = ["tz1", "tz2", "tz3", "KT1"];

impl FromStr for NewAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match &s[..3] {
            "KT1" => Ok(NewAddress::SF(SmartFunctionHash::from_str(s)?)),
            "tz1" | "tz2" | "tz3" => Ok(NewAddress::User(PublicKeyHash::from_str(s)?)),
            _ => Err(Error::InvalidAddress),
        }
    }
}

impl NewAddress {
    pub fn check_is_user(&self) -> Result<()> {
        match self {
            Self::User(_) => Ok(()),
            _ => Err(Error::InvalidAddress),
        }
    }

    pub fn check_is_smart_function(&self) -> Result<()> {
        match self {
            Self::SF(_) => Ok(()),
            _ => Err(Error::InvalidAddress),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_crypto::hash::JstzHash;

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
            NewAddress::SF(hash) if hash.to_base58() == KT1
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
    fn test_type_checks() {
        // Test tz1 type checks
        let tz1_addr = NewAddress::from_str(TZ1).unwrap();
        assert!(tz1_addr.check_is_user().is_ok());
        assert!(tz1_addr.check_is_smart_function().is_err());

        // Test tz2 type checks
        let tz2_addr = NewAddress::from_str(TZ2).unwrap();
        assert!(tz2_addr.check_is_user().is_ok());
        assert!(tz2_addr.check_is_smart_function().is_err());

        // Test tz3 type checks
        let tz3_addr = NewAddress::from_str(TZ3).unwrap();
        assert!(tz3_addr.check_is_user().is_ok());
        assert!(tz3_addr.check_is_smart_function().is_err());

        // Test KT1 type checks
        let kt1_addr = NewAddress::from_str(KT1).unwrap();
        assert!(kt1_addr.check_is_user().is_err());
        assert!(kt1_addr.check_is_smart_function().is_ok());
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
}
