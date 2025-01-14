use std::{
    fmt::{self, Display},
    result,
};

use crate::error::{Error, Result};
use bincode::{Decode, Encode};
use boa_engine::{Context, JsError, JsResult, Module, Source};
use jstz_core::{
    host::HostRuntime,
    kv::{Entry, Transaction},
};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};
use utoipa::ToSchema;

use super::new_account::NewAddress;

pub type Amount = u64;

#[derive(
    Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema,
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
#[derive(Default, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Account {
    #[bincode(with_serde)]
    pub nonce: Nonce,
    #[bincode(with_serde)]
    pub amount: Amount,
    #[bincode(with_serde)]
    pub function_code: Option<ParsedCode>,
}

const ACCOUNTS_PATH: RefPath = RefPath::assert_from(b"/jstz_account");

impl Account {
    pub fn path(pkh: &NewAddress) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", pkh))?;

        Ok(path::concat(&ACCOUNTS_PATH, &account_path)?)
    }

    fn get_mut<'a, 'b>(
        hrt: &impl HostRuntime,
        tx: &'b mut Transaction,
        addr: &NewAddress,
    ) -> Result<&'b mut Self>
    where
        'a: 'b,
    {
        let account_entry = tx.entry::<Self>(hrt, Self::path(addr)?)?;
        Ok(account_entry.or_insert_default())
    }

    fn try_insert(
        self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
    ) -> Result<()> {
        match tx.entry::<Self>(hrt, Self::path(addr)?)? {
            Entry::Occupied(ntry) => {
                let acc: &Self = ntry.get();
                hrt.write_debug(&format!("📜 already exists: {:?}\n", acc.function_code));
                Err(Error::InvalidAddress)
            }
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
        Ok(&mut account.nonce)
    }

    pub fn function_code<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &NewAddress,
    ) -> Result<Option<&'a mut String>> {
        let account = Self::get_mut(hrt, tx, addr)?;
        let function_code = account.function_code.as_mut().map(|code| &mut code.0);
        Ok(function_code)
    }

    pub fn set_function_code(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        function_code: String,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;
        account.function_code = Some(function_code.try_into()?);
        Ok(())
    }

    pub fn balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
    ) -> Result<Amount> {
        let account = Self::get_mut(hrt, tx, addr)?;
        Ok(account.amount)
    }

    pub fn add_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<u64> {
        let account = Self::get_mut(hrt, tx, addr)?;
        let checked_balance = account
            .amount
            .checked_add(amount)
            .ok_or(crate::error::Error::BalanceOverflow)?;

        account.amount = checked_balance;
        Ok(account.amount)
    }

    pub fn sub_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<u64> {
        let account = Self::get_mut(hrt, tx, addr)?;
        if account.amount < amount {
            return Err(Error::InsufficientFunds)?;
        }
        account.amount -= amount;
        Ok(account.amount)
    }

    pub fn set_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;
        account.amount = amount;
        Ok(())
    }

    pub fn create(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &NewAddress,
        amount: Amount,
        function_code: Option<ParsedCode>,
    ) -> Result<()> {
        Self {
            nonce: Nonce::default(),
            amount,
            function_code,
        }
        .try_insert(hrt, tx, addr)
    }

    pub fn transfer(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        src: &NewAddress,
        dst: &NewAddress,
        amt: Amount,
    ) -> Result<()> {
        {
            let src = tx
                .entry::<Account>(hrt, Self::path(src)?)?
                .or_insert_default();
            match src.amount.checked_sub(amt) {
                Some(amt) => src.amount = amt,
                None => return Err(Error::BalanceOverflow),
            }
        }

        {
            let dst = Self::get_mut(hrt, tx, dst)?;
            match dst.amount.checked_add(amt) {
                Some(amt) => dst.amount = amt,
                None => return Err(Error::BalanceOverflow),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    fn test_zero_account_balance_for_new_accounts() -> Result<()> {
        let hrt = &mut MockHost::default();
        let tx = &mut Transaction::default();

        tx.begin();

        let pkh = PublicKeyHash::from_base58("tz1XQjK1b3P72kMcHsoPhnAg3dvX1n8Ainty")
            .expect("Could not parse pkh");

        // Act
        let amt = {
            tx.entry::<Account>(hrt, Account::path(&NewAddress::User(pkh))?)?
                .or_insert_default()
                .amount
        };
        {
            tx.commit(hrt).expect("Could not commit tx");
        }

        // Assert
        assert_eq!(amt, 0);

        Ok(())
    }

    #[test]
    fn test_sub_balance() {
        let hrt = &mut MockHost::default();
        let tx = &mut Transaction::default();

        tx.begin();

        let pkh = NewAddress::User(
            PublicKeyHash::from_base58("tz1XQjK1b3P72kMcHsoPhnAg3dvX1n8Ainty")
                .expect("Could not parse pkh"),
        );

        Account::create(hrt, tx, &pkh, 10, None).unwrap();
        Account::sub_balance(hrt, tx, &pkh, 10).unwrap();

        assert_eq!(0, Account::balance(hrt, tx, &pkh).unwrap());

        let result = Account::sub_balance(hrt, tx, &pkh, 11);

        assert!(matches!(result, Err(Error::InsufficientFunds)));
    }
}
