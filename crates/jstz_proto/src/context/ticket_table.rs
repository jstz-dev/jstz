use crate::error::Result;
use derive_more::{Display, Error, From};
use jstz_core::kv::{Entry, Transaction};
use jstz_crypto::{hash::Blake2b, public_key_hash::PublicKeyHash};
use tezos_smart_rollup::{
    host::Runtime,
    storage::path::{self, OwnedPath, RefPath},
};

use super::account::Amount;

#[derive(Display, Debug, Error, From)]
pub enum TicketTableError {
    InsufficientFunds,
    AccountNotFound,
    TicketHashNotFound,
}

const TICKET_TABLE_PATH: RefPath = RefPath::assert_from(b"/ticket_table");

pub struct TicketTable;

impl TicketTable {
    fn path(ticket_hash: &Blake2b, owner: &PublicKeyHash) -> Result<OwnedPath> {
        let ticket_hash_path =
            OwnedPath::try_from(format!("/{}", ticket_hash.to_string()))?;
        let owner_path = OwnedPath::try_from(format!("/{}", owner))?;

        Ok(path::concat(
            &TICKET_TABLE_PATH,
            &path::concat(&ticket_hash_path, &owner_path)?,
        )?)
    }

    pub fn get_balance(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &PublicKeyHash,
        ticket_hash: &Blake2b,
    ) -> Result<Amount> {
        let path = Self::path(ticket_hash, owner)?;
        let result = tx.get::<Amount>(rt, path)?;
        match result {
            Some(balance) => Ok(*balance),
            None => Ok(0),
        }
    }

    pub fn add(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &PublicKeyHash,
        ticket_hash: &Blake2b,
        amount: Amount, // TODO: check if its the correct size
    ) -> Result<Amount> {
        let path = Self::path(ticket_hash, owner)?;
        match tx.entry::<Amount>(rt, path)? {
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(amount);
                Ok(amount)
            }
            Entry::Occupied(mut occupied) => {
                let balance = occupied.get_mut();
                let checked_balance = balance
                    .checked_add(amount)
                    .ok_or(crate::error::Error::BalanceOverflow)?;
                *balance = checked_balance;
                Ok(checked_balance)
            }
        }
    }

    pub fn sub(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &PublicKeyHash,
        ticket_hash: &Blake2b,
        amount: u64,
    ) -> Result<Amount> {
        let path = Self::path(ticket_hash, owner)?;
        match tx.entry::<Amount>(rt, path)? {
            Entry::Vacant(_) => Err(TicketTableError::AccountNotFound)?,
            Entry::Occupied(mut occupied) => {
                let balance = occupied.get_mut();
                if *balance < amount {
                    return Err(TicketTableError::InsufficientFunds)?;
                }
                *balance -= amount;
                Ok(*balance)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::TicketTable;
    use jstz_core::kv::Transaction;
    use jstz_mock::mock::{self, JstzMockHost};
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    fn path_format() {
        let ticket_hash = mock::ticket_hash1();
        let owner = mock::account1();
        let result = TicketTable::path(&ticket_hash, &owner).unwrap();
        let expectecd = "/ticket_table/4f3b771750d60ed12c38f5f80683fb53b37e3da02dd7381454add8f1dbd2ee60/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
        assert_eq!(expectecd, result.to_string());
    }

    #[test]
    fn add_tickets_succeeds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();
        let amount = 100;
        TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, amount).unwrap();
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(100, balance);

        let another_amount = 50;
        TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, another_amount)
            .unwrap();
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(150, balance);
    }

    #[test]
    fn add_tickets_overflow_fails() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();
        let amount = u64::MAX;
        TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, amount).unwrap();
        let err = TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, 1)
            .expect_err("Expected error");
        assert_eq!("BalanceOverflow", err.to_string());
    }

    #[test]
    fn sub_tickets_succeeds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();
        let amount = 100;
        TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, amount).unwrap();
        let another_amount = 70;
        TicketTable::sub(host.rt(), &mut tx, &owner, &ticket_hash, another_amount)
            .unwrap();
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(30, balance);
    }

    #[test]
    fn sub_tickets_account_not_found_fails() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();
        let amount = 100;
        let err = TicketTable::sub(host.rt(), &mut tx, &owner, &ticket_hash, amount)
            .expect_err("Expected error");
        assert_eq!(err.to_string(), "AccountNotFound");
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(0, balance);
    }

    #[test]
    fn update_tickets_in_succession() {
        let mut rt = MockHost::default();
        let mut tx = Transaction::default();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();

        let add_100 = 100;
        let sub_30 = 30;
        let add_25 = 25;
        let sub_25 = 25;
        let add_60 = 60;

        tx.begin();
        TicketTable::add(&mut rt, &mut tx, &owner, &ticket_hash, add_100).unwrap();
        tx.commit(&mut rt).unwrap();
        tx.begin();
        TicketTable::sub(&mut rt, &mut tx, &owner, &ticket_hash, sub_30).unwrap();
        TicketTable::add(&mut rt, &mut tx, &owner, &ticket_hash, add_25).unwrap();
        tx.begin();
        TicketTable::sub(&mut rt, &mut tx, &owner, &ticket_hash, sub_25).unwrap();
        TicketTable::add(&mut rt, &mut tx, &owner, &ticket_hash, add_60).unwrap();
        tx.commit(&mut rt).unwrap();
        tx.commit(&mut rt).unwrap();
        tx.begin();
        let result =
            TicketTable::get_balance(&mut rt, &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(130, result)
    }
}
