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

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Account {
    pub nonce: Nonce,
    amount: Amount,
    pub contract_code: Option<String>,
}

const ACCOUNTS_PATH: RefPath = RefPath::assert_from(b"/jstz_account");

impl Account {
    pub fn path(pkh: &Address) -> Result<OwnedPath> {
        let account_path = OwnedPath::try_from(format!("/{}", pkh))?;

        Ok(path::concat(&ACCOUNTS_PATH, &account_path)?)
    }

    fn get_mut<'a, 'b>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &Address,
    ) -> Result<&'b mut Account>
    where
        'a: 'b,
    {
        let account_entry = tx.entry(hrt, Self::path(addr)?)?;

        Ok(account_entry.or_insert_default())
    }

    fn try_insert(
        self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
    ) -> Result<()> {
        match tx.entry(hrt, Self::path(addr)?)? {
            Entry::Occupied(ntry) => {
                let acc: &Self = ntry.get();
                hrt.write_debug(&format!("📜 already exists: {:?}\n", acc.contract_code));
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

    pub fn contract_code<'a>(
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        addr: &Address,
    ) -> Result<Option<&'a mut String>> {
        let account = Self::get_mut(hrt, tx, addr)?;

        Ok(account.contract_code.as_mut())
    }

    pub fn set_contract_code(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        addr: &Address,
        contract_code: String,
    ) -> Result<()> {
        let account = Self::get_mut(hrt, tx, addr)?;

        account.contract_code = Some(contract_code);
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
        contract_code: Option<String>,
    ) -> Result<()> {
        Self {
            nonce: Nonce::default(),
            amount,
            contract_code,
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
        let src = Self::get_mut(hrt, tx, src)?;
        match src.amount.checked_sub(amt) {
            Some(amt) => src.amount = amt,
            None => return Err(Error::BalanceOverflow),
        }

        let dst = Self::get_mut(hrt, tx, dst)?;
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
        let hrt = &mut MockHost::default();
        let mut kv = Kv::new();

        let mut tx = kv.begin_transaction();

        let pkh = PublicKeyHash::from_base58("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q")
            .expect("Could not parse pkh");

        // Act
        let amt = Account::balance(hrt, &mut tx, &pkh).expect("Could not get balance");

        kv.commit_transaction(hrt, tx).expect("Could not commit tx");

        // Assert
        assert_eq!(amt, 0);
    }
}
