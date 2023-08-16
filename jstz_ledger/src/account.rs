use crate::{
    error::{Error, Result},
    nonce::Nonce,
};
use jstz_core::kv::{self, Transaction};
use jstz_crypto::public_key_hash::PublicKeyHash;

use serde::{Deserialize, Serialize};
use tezos_smart_rollup_host::{
    path::{self, OwnedPath, RefPath},
    runtime::Runtime,
};

pub type Amount = u64;

#[derive(Debug, Serialize, Deserialize)]
pub struct Account {
    nonce: Nonce,
    amount: Amount,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            nonce: Default::default(),
            amount: Default::default(),
        }
    }
}

impl kv::Value for Account {}

const ACCOUNTS_PATH: RefPath = RefPath::assert_from(b"/jstz_account");

impl Account {
    pub fn path(pkh: PublicKeyHash) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", pkh))?;

        Ok(path::concat(&ACCOUNTS_PATH, &account_path)?)
    }

    fn get_mut<'a, 'b>(
        rt: &impl Runtime,
        tx: &'a mut Transaction,
        pkh: PublicKeyHash,
    ) -> Result<&'b mut Account>
    where
        'a: 'b,
    {
        let account_entry = tx.entry(rt, Self::path(pkh)?)?;

        Ok(account_entry.or_insert_default())
    }

    pub fn nonce<'a>(
        rt: &impl Runtime,
        tx: &'a mut Transaction,
        pkh: PublicKeyHash,
    ) -> Result<&'a Nonce> {
        let account = Self::get_mut(rt, tx, pkh)?;

        Ok(&mut account.nonce)
    }

    pub fn balance(
        rt: &impl Runtime,
        tx: &mut Transaction,
        pkh: PublicKeyHash,
    ) -> Result<Amount> {
        let account = Self::get_mut(rt, tx, pkh)?;

        Ok(account.amount)
    }

    pub fn transfer(
        rt: &impl Runtime,
        tx: &mut Transaction,
        src: PublicKeyHash,
        dst: PublicKeyHash,
        amt: Amount,
    ) -> Result<()> {
        let src = Self::get_mut(rt, tx, src)?;
        match src.amount.checked_sub(amt) {
            Some(amt) => src.amount = amt,
            None => return Err(Error::BalanceOverflow),
        }

        let dst = Self::get_mut(rt, tx, dst)?;
        match dst.amount.checked_add(amt) {
            Some(amt) => dst.amount = amt,
            None => return Err(Error::BalanceOverflow),
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jstz_core::kv::Kv;
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    fn test_zero_account_balance_for_new_accounts() {
        let mut rt = MockHost::default();
        let mut kv = Kv::new();

        let mut tx = kv.begin_transaction();

        let pkh = PublicKeyHash::from_base58("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q")
            .expect("Could not parse pkh");

        // Act
        let amt = Account::balance(&mut rt, &mut tx, pkh).expect("Could not get balance");

        kv.commit_transaction(&mut rt, tx)
            .expect("Could not commit tx");

        // Assert
        assert_eq!(amt, 0);
    }
}
