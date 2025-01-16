use std::{
    fmt::{self, Display},
    result,
    str::FromStr,
};

use crate::error::{Error, Result};
use bincode::{Decode, Encode};
use boa_engine::{Context, JsError, JsResult, Module, Source};
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

pub type Amount = u64;

#[derive(
    Clone,
    Copy,
    Default,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ToSchema,
    Encode,
    Decode,
)]
pub struct Nonce(pub u64);

impl Nonce {
    pub fn next(&self) -> Nonce {
        Nonce(self.0 + 1)
    }

    pub fn increment(&mut self) {
        self.0 += 1
    }
}

impl Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// Invariant: if code is present it parses successfully
#[derive(
    Default, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, ToSchema, Encode, Decode,
)]
#[schema(
    format = "javascript",
    example = "export default (request) => new Response('Hello world!')"
)]
pub struct ParsedCode(pub String);
impl From<ParsedCode> for String {
    fn from(ParsedCode(code): ParsedCode) -> Self {
        code
    }
}
impl Display for ParsedCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        Display::fmt(&self.0, formatter)
    }
}
impl TryFrom<String> for ParsedCode {
    type Error = JsError;
    fn try_from(code: String) -> JsResult<Self> {
        let src = Source::from_bytes(code.as_bytes());
        let mut context = Context::default();
        Module::parse(src, None, &mut context)?;
        Ok(Self(code))
    }
}

#[derive(
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
// TODO: rename to Address
// https://linear.app/tezos/issue/JSTZ-253/remove-old-accountrs
#[schema(as = Address)]
#[schema(description = "Tezos Address")]
#[serde(untagged)]
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

pub const ACCOUNTS_PATH_PREFIX: &str = "/jstz_account";
const ACCOUNTS_PATH: RefPath = RefPath::assert_from(ACCOUNTS_PATH_PREFIX.as_bytes());

#[derive(Debug, Default, Clone, Serialize, Deserialize, Encode, Decode, ToSchema)]
pub struct UserAccount {
    pub amount: Amount,
    pub nonce: Nonce,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Encode, Decode, ToSchema)]
pub struct SmartFunctionAccount {
    pub amount: Amount,
    pub nonce: Nonce,
    pub function_code: ParsedCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, ToSchema)]
pub enum Account {
    User(UserAccount),
    SmartFunction(SmartFunctionAccount),
}

impl Account {
    fn path(addr: &NewAddress) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", addr))?;
        Ok(path::concat(&ACCOUNTS_PATH, &account_path)?)
    }

    fn default_account(addr: &NewAddress) -> Self {
        match addr {
            NewAddress::User(_) => Self::User(UserAccount::default()),
            NewAddress::SmartFunction(_) => {
                Self::SmartFunction(SmartFunctionAccount::default())
            }
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
            (Account::User(_), NewAddress::User(_))
            | (Account::SmartFunction(_), NewAddress::SmartFunction(_)) => {}
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

    fn balance_mut<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'a mut Amount> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::User(UserAccount { amount, .. }) => Ok(amount),
            Self::SmartFunction(SmartFunctionAccount { amount, .. }) => Ok(amount),
        }
    }

    pub fn nonce<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'a mut Nonce> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::User(UserAccount { nonce, .. }) => Ok(nonce),
            Self::SmartFunction(SmartFunctionAccount { nonce, .. }) => Ok(nonce),
        }
    }

    pub fn create_smart_function(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        creator: &NewAddress,
        amount: Amount,
        function_code: ParsedCode,
        // TODO: return smart function hash
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
    ) -> Result<NewAddress> {
        let nonce = Self::nonce(hrt, tx, creator)?;
        let address = NewAddress::SmartFunction(SmartFunctionHash::digest(
            format!("{}{}{}", creator, function_code, nonce).as_bytes(),
        )?);
        let account = SmartFunctionAccount {
            amount,
            nonce: Nonce::default(),
            function_code,
        };
        Self::SmartFunction(account).try_insert(hrt, tx, &address)?;
        Ok(address)
    }

    pub fn function_code<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        // TODO: use smart function hash
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        addr: &NewAddress,
    ) -> Result<&'a str> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::SmartFunction(SmartFunctionAccount { function_code, .. }) => {
                Ok(&function_code.0)
            }
            Self::User(_) => Err(Error::AddressTypeMismatch),
        }
    }

    // TODO: Used only in repl, conditionally compile
    // https://linear.app/tezos/issue/JSTZ-282/conditionally-compile-for-repl
    pub fn set_function_code(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        // TODO: use smart function hash
        // https://linear.app/tezos/issue/JSTZ-260/add-validation-check-for-address-type
        addr: &NewAddress,
        new_function_code: String,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;
        match account {
            Self::SmartFunction(SmartFunctionAccount { function_code, .. }) => {
                *function_code = new_function_code.try_into()?;
                Ok(())
            }
            Self::User(_) => Err(Error::AddressTypeMismatch),
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
        fn test_account_path() {
            let (user_addr, sf_addr) = create_test_addresses();

            // Test basic paths
            let user_path = Account::path(&user_addr).unwrap();
            assert_eq!(user_path.to_string(), format!("/jstz_account/{}", TZ1));

            let sf_path = Account::path(&sf_addr).unwrap();
            assert_eq!(sf_path.to_string(), format!("/jstz_account/{}", KT1));

            // Test path validation
            let addr = NewAddress::User(
                PublicKeyHash::from_base58("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
                    .unwrap(),
            );
            assert!(Account::path(&addr).is_ok());

            // Test path format
            let path = Account::path(&user_addr).unwrap();
            assert!(path.to_string().starts_with("/jstz_account/"));
            assert!(path.to_string().contains(TZ1));

            let path = Account::path(&sf_addr).unwrap();
            assert!(path.to_string().starts_with("/jstz_account/"));
            assert!(path.to_string().contains(KT1));
        }

        #[test]
        fn test_default_account() {
            let (user_addr, sf_addr) = create_test_addresses();

            match Account::default_account(&user_addr) {
                Account::User(account) => {
                    assert_eq!(account.amount, 0);
                    assert_eq!(account.nonce.0, 0);
                }
                _ => panic!("Expected User account"),
            }

            match Account::default_account(&sf_addr) {
                Account::SmartFunction(account) => {
                    assert_eq!(account.amount, 0);
                    assert_eq!(account.nonce.0, 0);
                    assert_eq!(account.function_code.0, "");
                }
                _ => panic!("Expected SmartFunction account"),
            }
        }

        #[test]
        fn test_get_mut() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            let user_account = Account::get_mut(&host, &mut tx, &user_addr).unwrap();
            match user_account {
                Account::User(account) => {
                    assert_eq!(account.amount, 0);
                    assert_eq!(account.nonce.0, 0);
                }
                _ => panic!("Expected User account"),
            }

            let sf_account = Account::get_mut(&host, &mut tx, &sf_addr).unwrap();
            match sf_account {
                Account::SmartFunction(account) => {
                    assert_eq!(account.amount, 0);
                    assert_eq!(account.nonce.0, 0);
                    assert_eq!(account.function_code.0, "");
                }
                _ => panic!("Expected SmartFunction account"),
            }
        }

        #[test]
        fn test_balance_operations() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            // Test initial balances
            assert_eq!(Account::balance(&host, &mut tx, &user_addr).unwrap(), 0);
            assert_eq!(Account::balance(&host, &mut tx, &sf_addr).unwrap(), 0);

            // Test adding balance
            assert_eq!(
                Account::add_balance(&host, &mut tx, &user_addr, 100).unwrap(),
                100
            );
            assert_eq!(
                Account::add_balance(&host, &mut tx, &sf_addr, 200).unwrap(),
                200
            );

            // Test balance overflow scenarios
            Account::set_balance(&host, &mut tx, &user_addr, u64::MAX).unwrap();
            assert!(matches!(
                Account::add_balance(&host, &mut tx, &user_addr, 1),
                Err(Error::BalanceOverflow)
            ));

            // Test subtracting balance
            Account::set_balance(&host, &mut tx, &user_addr, 100).unwrap();
            assert_eq!(
                Account::sub_balance(&host, &mut tx, &user_addr, 50).unwrap(),
                50
            );
            assert_eq!(
                Account::sub_balance(&host, &mut tx, &sf_addr, 100).unwrap(),
                100
            );

            // Test insufficient funds
            assert!(matches!(
                Account::sub_balance(&host, &mut tx, &user_addr, 1000),
                Err(Error::InsufficientFunds)
            ));

            // Test setting balance
            assert!(Account::set_balance(&host, &mut tx, &user_addr, 1000).is_ok());
            assert_eq!(Account::balance(&host, &mut tx, &user_addr).unwrap(), 1000);

            // Test overflow protection in transfer
            let (addr1, addr2) = create_test_addresses();
            Account::set_balance(&host, &mut tx, &addr1, 100).unwrap();
            Account::set_balance(&host, &mut tx, &addr2, u64::MAX - 50).unwrap();
            assert!(matches!(
                Account::transfer(&host, &mut tx, &addr1, &addr2, 100),
                Err(Error::BalanceOverflow)
            ));
        }

        #[test]
        fn test_transfer() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            Account::set_balance(&host, &mut tx, &user_addr, 1000).unwrap();
            Account::set_balance(&host, &mut tx, &sf_addr, 500).unwrap();

            assert!(Account::transfer(&host, &mut tx, &user_addr, &sf_addr, 300).is_ok());
            assert_eq!(Account::balance(&host, &mut tx, &user_addr).unwrap(), 700);
            assert_eq!(Account::balance(&host, &mut tx, &sf_addr).unwrap(), 800);

            assert!(matches!(
                Account::transfer(&host, &mut tx, &user_addr, &sf_addr, 1000),
                Err(Error::InsufficientFunds)
            ));

            Account::set_balance(&host, &mut tx, &sf_addr, u64::MAX).unwrap();
            assert!(matches!(
                Account::transfer(&host, &mut tx, &user_addr, &sf_addr, 1),
                Err(Error::BalanceOverflow)
            ));
        }

        #[test]
        fn test_function_code_operations() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            // Test type mismatch
            assert!(matches!(
                Account::function_code(&host, &mut tx, &user_addr),
                Err(Error::AddressTypeMismatch)
            ));

            // Test empty initial code
            let code = Account::function_code(&host, &mut tx, &sf_addr).unwrap();
            assert_eq!(code, "");

            // Test empty function code
            assert!(
                Account::set_function_code(&host, &mut tx, &sf_addr, "".to_string())
                    .is_ok()
            );

            // Test setting and retrieving valid code
            let valid_code = "function test() { return 42; }".to_string();
            assert!(Account::set_function_code(
                &host,
                &mut tx,
                &sf_addr,
                valid_code.clone()
            )
            .is_ok());
            let updated_code = Account::function_code(&host, &mut tx, &sf_addr).unwrap();
            assert_eq!(updated_code, valid_code);
        }

        #[test]
        fn test_try_insert() {
            let (host, mut tx) = setup_test_env();
            let (user_addr, sf_addr) = create_test_addresses();

            // Test inserting user account
            let user_account = Account::User(UserAccount {
                amount: 100,
                nonce: Nonce(0),
            });
            assert!(user_account.try_insert(&host, &mut tx, &user_addr).is_ok());

            let retrieved_user = Account::get_mut(&host, &mut tx, &user_addr).unwrap();
            match retrieved_user {
                Account::User(account) => {
                    assert_eq!(account.amount, 100);
                    assert_eq!(account.nonce.0, 0);
                }
                _ => panic!("Expected User account"),
            }

            // Test inserting smart function account
            let sf_account = Account::SmartFunction(SmartFunctionAccount {
                amount: 200,
                nonce: Nonce(0),
                function_code: ParsedCode("test_code".to_string()),
            });
            assert!(sf_account.try_insert(&host, &mut tx, &sf_addr).is_ok());

            let retrieved_sf = Account::get_mut(&host, &mut tx, &sf_addr).unwrap();
            match retrieved_sf {
                Account::SmartFunction(account) => {
                    assert_eq!(account.amount, 200);
                    assert_eq!(account.nonce.0, 0);
                    assert_eq!(account.function_code.0, "test_code");
                }
                _ => panic!("Expected SmartFunction account"),
            }

            // Test duplicate insertion should fail
            let duplicate_user = Account::User(UserAccount {
                amount: 300,
                nonce: Nonce(1),
            });
            assert!(matches!(
                duplicate_user.try_insert(&host, &mut tx, &user_addr),
                Err(Error::AccountExists)
            ));

            let duplicate_sf = Account::SmartFunction(SmartFunctionAccount {
                amount: 400,
                nonce: Nonce(1),
                function_code: ParsedCode("another_code".to_string()),
            });
            assert!(matches!(
                duplicate_sf.try_insert(&host, &mut tx, &sf_addr),
                Err(Error::AccountExists)
            ));

            // Test inserting with mismatched types
            let mismatched_user = Account::User(UserAccount {
                amount: 500,
                nonce: Nonce(0),
            });
            assert!(matches!(
                mismatched_user.try_insert(&host, &mut tx, &sf_addr),
                Err(Error::AddressTypeMismatch)
            ));

            let mismatched_sf = Account::SmartFunction(SmartFunctionAccount {
                amount: 600,
                nonce: Nonce(0),
                function_code: ParsedCode("code".to_string()),
            });
            assert!(matches!(
                mismatched_sf.try_insert(&host, &mut tx, &user_addr),
                Err(Error::AddressTypeMismatch)
            ));
        }

        #[test]
        fn test_nonce_increment() {
            let (mut host, mut tx) = setup_test_env();
            let (user_addr, _) = create_test_addresses();

            tx.begin();
            let nonce = Account::nonce(&host, &mut tx, &user_addr).unwrap();
            assert_eq!(nonce.0, 0);

            nonce.increment();
            tx.commit(&mut host).unwrap();

            tx.begin();
            let nonce = Account::nonce(&host, &mut tx, &user_addr).unwrap();
            assert_eq!(nonce.0, 1, "Nonce should be incremented and persisted");
        }

        #[test]
        fn test_create_smart_function() {
            let (host, mut tx) = setup_test_env();
            let (creator, _) = create_test_addresses();

            let code = ParsedCode("function test() {}".to_string());
            let amount = 100;

            // Create smart function account
            let address = Account::create_smart_function(
                &host,
                &mut tx,
                &creator,
                amount,
                code.clone(),
            )
            .unwrap();

            assert!(matches!(address, NewAddress::SmartFunction(_)));

            let account = Account::get_mut(&host, &mut tx, &address).unwrap();
            match account {
                Account::SmartFunction(sf_account) => {
                    assert_eq!(sf_account.amount, amount);
                    assert_eq!(sf_account.nonce.0, 0);
                    assert_eq!(sf_account.function_code.0, code.0);
                }
                _ => panic!("Expected SmartFunction account"),
            }
        }
    }
}
