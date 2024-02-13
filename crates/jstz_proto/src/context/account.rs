use std::fmt::Debug;

use crate::error::{Error, Result};
use jstz_core::{
    host::HostRuntime,
    kv::{Entry, Transaction},
};
use jstz_crypto::public_key_hash::PublicKeyHash;

use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};

pub type Address = PublicKeyHash;

pub type Amount = u64;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nonce(u64);

impl Nonce {
    pub fn next(&self) -> Nonce {
        Nonce(self.0 + 1)
    }

    pub fn increment(&mut self) {
        self.0 += 1
    }
}

impl ToString for Nonce {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub nonce: Nonce,
    pub amount: Amount,
    pub function_code: Option<String>,
}

const ACCOUNTS_PATH: RefPath = RefPath::assert_from(b"/jstz_account");

impl Account {
    pub fn path(pkh: &Address) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", pkh))?;

        Ok(path::concat(&ACCOUNTS_PATH, &account_path)?)
    }

    fn get_mut<'a, 'b>(
        hrt: &impl HostRuntime,
        tx: &'b mut Transaction,
        addr: &Address,
    ) -> Result<&'b mut Account>
    where
        'a: 'b,
    {
        let account_entry = tx.entry::<Account>(hrt, Self::path(addr)?)?;
        Ok(account_entry.or_insert_default())
    }

    fn try_insert(
        self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
    ) -> Result<()> {
        match tx.entry::<Account>(hrt, Self::path(addr)?)? {
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
        addr: &Address,
    ) -> Result<&'a mut Nonce> {
        let account = Self::get_mut(hrt, tx, addr)?;

        Ok(&mut account.nonce)
    }

    pub fn function_code<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &Address,
    ) -> Result<Option<&'a mut String>> {
        let account = Self::get_mut(hrt, tx, addr)?;
        Ok(account.function_code.as_mut())
    }

    pub fn set_function_code(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
        function_code: String,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;

        account.function_code = Some(function_code);
        Ok(())
    }

    pub fn balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
    ) -> Result<Amount> {
        let account = Self::get_mut(hrt, tx, addr)?;

        Ok(account.amount)
    }

    pub fn deposit(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
        amount: Amount,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;

        account.amount += amount;
        Ok(())
    }

    pub fn set_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
        amount: Amount,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;

        account.amount = amount;
        Ok(())
    }

    pub fn create(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
        amount: Amount,
        function_code: Option<String>,
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
        src: &Address,
        dst: &Address,
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
            tx.entry::<Account>(hrt, Account::path(&pkh)?)?
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
}
