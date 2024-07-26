use crate::error::Result;
use derive_more::{Display, Error, From};
use jstz_core::kv::Transaction;
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
    fn path(suffix: &OwnedPath) -> Result<OwnedPath> {
        Ok(path::concat(&TICKET_TABLE_PATH, suffix)?)
    }

    pub fn balance_add(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &PublicKeyHash,
        ticket_hash: &Blake2b,
        amount: Amount, // TODO: check if its the correct size
    ) -> Result<Amount> {
        let ticket_hash_path =
            OwnedPath::try_from(format!("/{}", ticket_hash.to_string()))?;
        let owner_path = OwnedPath::try_from(format!("/{}", owner))?;
        let path = Self::path(&path::concat(&ticket_hash_path, &owner_path)?)?;

        match tx.get::<Amount>(rt, path.clone())? {
            None => {
                tx.insert(path, amount)?;
                Ok(amount)
            }
            Some(balance) => {
                let checked_balance = balance
                    .checked_add(amount)
                    .ok_or(crate::error::Error::BalanceOverflow)?;
                tx.insert(path, checked_balance)?;
                Ok(checked_balance)
            }
        }
    }

    pub fn balance_sub(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &PublicKeyHash,
        ticket_hash: &Blake2b,
        amount: u64,
    ) -> Result<Amount> {
        let ticket_hash_path =
            OwnedPath::try_from(format!("/{}", ticket_hash.to_string()))?;
        let owner_path = OwnedPath::try_from(format!("/{}", owner))?;
        let path = Self::path(&path::concat(&ticket_hash_path, &owner_path)?)?;

        match tx.get::<Amount>(rt, path.clone())? {
            None => Err(TicketTableError::AccountNotFound)?,
            Some(balance) => {
                if *balance < amount {
                    return Err(TicketTableError::InsufficientFunds)?;
                }
                let final_balance = balance - amount;
                tx.insert(path, final_balance)?;
                Ok(final_balance)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use jstz_core::kv::{Storage, Transaction};
    use jstz_crypto::{hash::Blake2b, public_key_hash::PublicKeyHash};
    use jstz_mock::mock::{self, JstzMockHost};
    use tezos_smart_rollup::{host::Runtime, storage::path::OwnedPath};
    use tezos_smart_rollup_mock::MockHost;

    use super::TicketTable;

    fn get_ticket_value(
        rt: &mut impl Runtime,
        ticket_hash: &Blake2b,
        owner: &PublicKeyHash,
    ) -> Option<u64> {
        let path = TicketTable::path(
            &OwnedPath::try_from(format!("/{}/{}", ticket_hash.to_string(), owner))
                .unwrap(),
        )
        .unwrap();

        Storage::get::<u64>(rt, &path).unwrap()
    }

    #[test]
    fn add_tickets_succeeds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = mock::account1();
        let ticket_hash = mock::ticket_hash1();
        let amount = 100;
        TicketTable::balance_add(host.rt(), &mut tx, &owner, &ticket_hash, amount)
            .unwrap();
        tx.commit(host.rt()).unwrap();
        let balance = get_ticket_value(host.rt(), &ticket_hash, &owner).unwrap();
        assert_eq!(100, balance);

        tx.begin();
        let another_amount = 50;
        TicketTable::balance_add(
            host.rt(),
            &mut tx,
            &owner,
            &ticket_hash,
            another_amount,
        )
        .unwrap();
        tx.commit(host.rt()).unwrap();
        let balance = get_ticket_value(host.rt(), &ticket_hash, &owner).unwrap();
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
        TicketTable::balance_add(host.rt(), &mut tx, &owner, &ticket_hash, amount)
            .unwrap();
        let err = TicketTable::balance_add(host.rt(), &mut tx, &owner, &ticket_hash, 1)
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
        TicketTable::balance_add(host.rt(), &mut tx, &owner, &ticket_hash, amount)
            .unwrap();
        let another_amount = 70;
        TicketTable::balance_sub(
            host.rt(),
            &mut tx,
            &owner,
            &ticket_hash,
            another_amount,
        )
        .unwrap();
        tx.commit(host.rt()).unwrap();
        let balance = get_ticket_value(host.rt(), &ticket_hash, &owner).unwrap();
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
        let err =
            TicketTable::balance_sub(host.rt(), &mut tx, &owner, &ticket_hash, amount)
                .expect_err("Expected error");
        assert_eq!(err.to_string(), "AccountNotFound");
        let balance = get_ticket_value(host.rt(), &ticket_hash, &owner);
        assert_eq!(None, balance);
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
        TicketTable::balance_add(&mut rt, &mut tx, &owner, &ticket_hash, add_100)
            .unwrap();
        tx.commit(&mut rt).unwrap();
        tx.begin();
        TicketTable::balance_sub(&mut rt, &mut tx, &owner, &ticket_hash, sub_30).unwrap();
        TicketTable::balance_add(&mut rt, &mut tx, &owner, &ticket_hash, add_25).unwrap();
        tx.begin();
        TicketTable::balance_sub(&mut rt, &mut tx, &owner, &ticket_hash, sub_25).unwrap();
        TicketTable::balance_add(&mut rt, &mut tx, &owner, &ticket_hash, add_60).unwrap();
        tx.commit(&mut rt).unwrap();
        tx.commit(&mut rt).unwrap();
        let result = get_ticket_value(&mut rt, &ticket_hash, &owner).unwrap();
        assert_eq!(130, result)
    }
}
