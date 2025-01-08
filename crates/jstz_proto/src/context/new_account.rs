use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crate::error::{Error, Result};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::{
    host::HostRuntime,
    kv::{Entry, Transaction},
};
use jstz_crypto::hash::Hash;
use jstz_crypto::public_key_hash::PublicKeyHash;
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};
use utoipa::ToSchema;

use super::account::{Amount, Nonce, ParsedCode};

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
    pub fn check_is_smart_function(&self) -> Result<()> {
        match self {
            Self::SmartFunction(_) => Ok(()),
            _ => Err(Error::AddressTypeMismatch),
        }
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

const NEW_ACCOUNTS_PATH: RefPath = RefPath::assert_from(b"/jstz_new_account");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NewAccount {
    User {
        amount: Amount,
        nonce: Nonce,
    },
    SmartFunction {
        amount: Amount,
        nonce: Nonce,
        function_code: ParsedCode,
    },
}

impl NewAccount {
    fn path(addr: &NewAddress) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", addr))?;
        Ok(path::concat(&NEW_ACCOUNTS_PATH, &account_path)?)
    }

    fn default_account(addr: &NewAddress) -> Self {
        match addr {
            NewAddress::User(_) => Self::User {
                amount: Amount::default(),
                nonce: Nonce::default(),
            },
            NewAddress::SmartFunction(_) => Self::SmartFunction {
                amount: Amount::default(),
                nonce: Nonce::default(),
                function_code: ParsedCode::default(),
            },
        }
    }

    fn get_mut<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'a mut Self> {
        let account_entry = tx.entry::<Self>(hrt, Self::path(addr)?)?;
        Ok(account_entry.or_insert_with(|| Self::default_account(addr)))
    }

    fn try_insert(
        self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
    ) -> Result<()> {
        match (&self, addr) {
            (NewAccount::User { .. }, NewAddress::User(_))
            | (NewAccount::SmartFunction { .. }, NewAddress::SmartFunction(_)) => {}
            _ => return Err(Error::AddressTypeMismatch),
        }

        match tx.entry::<Self>(hrt, Self::path(addr)?)? {
            Entry::Occupied(_) => Err(Error::AccountExists),
            Entry::Vacant(entry) => {
                entry.insert(self);
                Ok(())
            }
        }
    }

    pub fn nonce<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'a mut Nonce> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::User { nonce, .. } => Ok(nonce),
            Self::SmartFunction { nonce, .. } => Ok(nonce),
        }
    }

    pub fn function_code<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        // TODO: use smart function hash
        // https://linear.app/tezos/issue/JSTZ-271/use-smart-function-hash-for-new-account
        addr: &NewAddress,
    ) -> Result<&'a str> {
        addr.check_is_smart_function()?;
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::SmartFunction { function_code, .. } => Ok(&function_code.0),
            _ => Err(Error::AddressTypeMismatch),
        }
    }

    pub fn set_function_code(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        // TODO: use smart function hash
        // https://linear.app/tezos/issue/JSTZ-271/use-smart-function-hash-for-new-account
        addr: &NewAddress,
        new_function_code: String,
    ) -> Result<()> {
        addr.check_is_smart_function()?;
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::SmartFunction { function_code, .. } => {
                *function_code = new_function_code.try_into()?;
                Ok(())
            }
            _ => Err(Error::AddressTypeMismatch),
        }
    }

    fn balance_mut<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'a mut Amount> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::SmartFunction { amount, .. } => Ok(amount),
            Self::User { amount, .. } => Ok(amount),
        }
    }

    pub fn balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
    ) -> Result<Amount> {
        let balance = Self::balance_mut(hrt, tx, addr)?;
        Ok(*balance)
    }

    pub fn add_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<u64> {
        let balance = Self::balance_mut(hrt, tx, addr)?;
        let checked_balance = balance
            .checked_add(amount)
            .ok_or(crate::error::Error::BalanceOverflow)?;

        *balance = checked_balance;
        Ok(checked_balance)
    }

    pub fn sub_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<u64> {
        let balance = Self::balance_mut(hrt, tx, addr)?;
        if *balance < amount {
            return Err(Error::InsufficientFunds)?;
        }
        *balance -= amount;
        Ok(*balance)
    }

    pub fn set_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<()> {
        let balance = Self::balance_mut(hrt, tx, addr)?;
        *balance = amount;
        Ok(())
    }

    pub fn create_smart_function(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        // TODO: use smart function hash
        // https://linear.app/tezos/issue/JSTZ-271/use-smart-function-hash-for-new-account
        addr: &NewAddress,
        amount: Amount,
        function_code: ParsedCode,
    ) -> Result<()> {
        addr.check_is_smart_function()?;
        Self::SmartFunction {
            amount,
            nonce: Nonce::default(),
            function_code,
        }
        .try_insert(hrt, tx, addr)
    }

    pub fn transfer(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        src: &NewAddress,
        dst: &NewAddress,
        amount: Amount,
    ) -> Result<()> {
        let src_balance = Self::balance_mut(hrt, tx, src)?;
        match src_balance.checked_sub(amount) {
            Some(amt) => *src_balance = amt,
            None => return Err(Error::InsufficientFunds),
        }

        let dst_balance = Self::balance_mut(hrt, tx, dst)?;
        match dst_balance.checked_add(amount) {
            Some(amt) => *dst_balance = amt,
            None => return Err(Error::BalanceOverflow),
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_core::kv::Transaction;
    use jstz_crypto::hash::Hash;
    use tezos_smart_rollup_mock::MockHost;

    // Test constants
    const TZ1: &str = "tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU";
    const TZ2: &str = "tz2KDvEL9fuvytRfe1cVVDo1QfDfaBktGNkh";
    const TZ3: &str = "tz3QxNCB8HgxJyp5V9ZmCVGcTm6BzYc14k9C";
    const KT1: &str = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";

    mod self_address {
        use super::*;
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
        fn test_type_checks() {
            // Test tz1 type checks
            let tz1_addr = NewAddress::from_str(TZ1).unwrap();
            assert!(tz1_addr.check_is_smart_function().is_err());

            // Test tz2 type checks
            let tz2_addr = NewAddress::from_str(TZ2).unwrap();
            assert!(tz2_addr.check_is_smart_function().is_err());

            // Test tz3 type checks
            let tz3_addr = NewAddress::from_str(TZ3).unwrap();
            assert!(tz3_addr.check_is_smart_function().is_err());

            // Test KT1 type checks
            let kt1_addr = NewAddress::from_str(KT1).unwrap();
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

    mod account {
        use super::*;

        fn setup_test_env() -> (MockHost, Transaction) {
            let host = MockHost::default();
            let mut tx = Transaction::default();
            tx.begin();
            (host, tx)
        }

        fn create_test_addresses() -> (NewAddress, NewAddress) {
            let user_addr = NewAddress::from_str(TZ1).unwrap();
            let sf_addr = NewAddress::from_str(KT1).unwrap();
            (user_addr, sf_addr)
        }

        #[test]
        fn test_new_account_path() {
            let (user_addr, sf_addr) = create_test_addresses();

            let user_path = NewAccount::path(&user_addr).unwrap();
            assert_eq!(user_path.to_string(), format!("/jstz_new_account/{}", TZ1));

            let sf_path = NewAccount::path(&sf_addr).unwrap();
            assert_eq!(sf_path.to_string(), format!("/jstz_new_account/{}", KT1));
        }

        #[test]
        fn test_default_account() {
            let (user_addr, sf_addr) = create_test_addresses();

            match NewAccount::default_account(&user_addr) {
                NewAccount::User { amount, nonce } => {
                    assert_eq!(amount, 0);
                    assert_eq!(nonce.0, 0);
                }
                _ => panic!("Expected User account"),
            }

            match NewAccount::default_account(&sf_addr) {
                NewAccount::SmartFunction {
                    amount,
                    nonce,
                    function_code,
                } => {
                    assert_eq!(amount, 0);
                    assert_eq!(nonce.0, 0);
                    assert_eq!(function_code.0, "");
                }
                _ => panic!("Expected SmartFunction account"),
            }
        }

        #[test]
        fn test_get_mut() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            let user_account = NewAccount::get_mut(&host, &mut tx, &user_addr).unwrap();
            match user_account {
                NewAccount::User { amount, nonce } => {
                    assert_eq!(*amount, 0);
                    assert_eq!(nonce.0, 0);
                }
                _ => panic!("Expected User account"),
            }

            let sf_account = NewAccount::get_mut(&host, &mut tx, &sf_addr).unwrap();
            match sf_account {
                NewAccount::SmartFunction {
                    amount,
                    nonce,
                    function_code,
                } => {
                    assert_eq!(*amount, 0);
                    assert_eq!(nonce.0, 0);
                    assert_eq!(function_code.0, "");
                }
                _ => panic!("Expected SmartFunction account"),
            }
        }

        #[test]
        fn test_balance_operations() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            assert_eq!(NewAccount::balance(&host, &mut tx, &user_addr).unwrap(), 0);
            assert_eq!(NewAccount::balance(&host, &mut tx, &sf_addr).unwrap(), 0);

            assert_eq!(
                NewAccount::add_balance(&host, &mut tx, &user_addr, 100).unwrap(),
                100
            );
            assert_eq!(
                NewAccount::add_balance(&host, &mut tx, &sf_addr, 200).unwrap(),
                200
            );

            assert!(matches!(
                NewAccount::add_balance(&host, &mut tx, &user_addr, u64::MAX),
                Err(Error::BalanceOverflow)
            ));

            assert_eq!(
                NewAccount::sub_balance(&host, &mut tx, &user_addr, 50).unwrap(),
                50
            );
            assert_eq!(
                NewAccount::sub_balance(&host, &mut tx, &sf_addr, 100).unwrap(),
                100
            );

            assert!(matches!(
                NewAccount::sub_balance(&host, &mut tx, &user_addr, 1000),
                Err(Error::InsufficientFunds)
            ));

            assert!(NewAccount::set_balance(&host, &mut tx, &user_addr, 1000).is_ok());
            assert_eq!(
                NewAccount::balance(&host, &mut tx, &user_addr).unwrap(),
                1000
            );
        }

        #[test]
        fn test_transfer() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            NewAccount::set_balance(&host, &mut tx, &user_addr, 1000).unwrap();
            NewAccount::set_balance(&host, &mut tx, &sf_addr, 500).unwrap();

            assert!(
                NewAccount::transfer(&host, &mut tx, &user_addr, &sf_addr, 300).is_ok()
            );
            assert_eq!(
                NewAccount::balance(&host, &mut tx, &user_addr).unwrap(),
                700
            );
            assert_eq!(NewAccount::balance(&host, &mut tx, &sf_addr).unwrap(), 800);

            assert!(matches!(
                NewAccount::transfer(&host, &mut tx, &user_addr, &sf_addr, 1000),
                Err(Error::InsufficientFunds)
            ));

            NewAccount::set_balance(&host, &mut tx, &sf_addr, u64::MAX).unwrap();
            assert!(matches!(
                NewAccount::transfer(&host, &mut tx, &user_addr, &sf_addr, 1),
                Err(Error::BalanceOverflow)
            ));
        }

        #[test]
        fn test_function_code_operations() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            assert!(matches!(
                NewAccount::function_code(&host, &mut tx, &user_addr),
                Err(Error::AddressTypeMismatch)
            ));

            let code = NewAccount::function_code(&host, &mut tx, &sf_addr).unwrap();
            assert_eq!(code, "");

            let new_code = "new_test_code";
            let _ = NewAccount::set_function_code(
                &host,
                &mut tx,
                &sf_addr,
                new_code.to_string(),
            );
            let updated_code =
                NewAccount::function_code(&host, &mut tx, &sf_addr).unwrap();
            assert_eq!(updated_code, new_code);
        }

        #[test]
        fn test_try_insert() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            // Test inserting user account
            let user_account = NewAccount::User {
                amount: 100,
                nonce: Nonce(0),
            };
            assert!(user_account.try_insert(&host, &mut tx, &user_addr).is_ok());

            // Test inserting smart function account
            let sf_account = NewAccount::SmartFunction {
                amount: 200,
                nonce: Nonce(0),
                function_code: ParsedCode("test_code".to_string()),
            };
            assert!(sf_account.try_insert(&host, &mut tx, &sf_addr).is_ok());

            // Test duplicate insertion should fail
            let duplicate_user = NewAccount::User {
                amount: 300,
                nonce: Nonce(1),
            };
            assert!(matches!(
                duplicate_user.try_insert(&host, &mut tx, &user_addr),
                Err(Error::AccountExists)
            ));

            let duplicate_sf = NewAccount::SmartFunction {
                amount: 400,
                nonce: Nonce(1),
                function_code: ParsedCode("another_code".to_string()),
            };
            assert!(matches!(
                duplicate_sf.try_insert(&host, &mut tx, &sf_addr),
                Err(Error::AccountExists)
            ));

            // Test inserting with mismatched types
            let mismatched_user = NewAccount::User {
                amount: 500,
                nonce: Nonce(0),
            };
            assert!(matches!(
                mismatched_user.try_insert(&host, &mut tx, &sf_addr),
                Err(Error::AddressTypeMismatch)
            ));

            let mismatched_sf = NewAccount::SmartFunction {
                amount: 600,
                nonce: Nonce(0),
                function_code: ParsedCode("code".to_string()),
            };
            assert!(matches!(
                mismatched_sf.try_insert(&host, &mut tx, &user_addr),
                Err(Error::AddressTypeMismatch)
            ));
        }
    }
}
