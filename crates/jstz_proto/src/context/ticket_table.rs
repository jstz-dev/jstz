use derive_more::{Display, Error, From};
use jstz_core::kv::{Entry, Transaction};
use tezos_smart_rollup::{
    host::Runtime,
    michelson::ticket::TicketHash,
    storage::path::{self, OwnedPath, RefPath},
};

use super::account::{Addressable, Amount};

use crate::error::Result;

#[derive(Display, Debug, Error, From)]
pub enum TicketTableError {
    InsufficientFunds,
    AccountNotFound,
    TicketHashNotFound,
}

const TICKET_TABLE_PATH: RefPath = RefPath::assert_from(b"/ticket_table");

pub struct TicketTable;

impl TicketTable {
    fn path(ticket_hash: &TicketHash, owner: &impl Addressable) -> Result<OwnedPath> {
        let ticket_hash_path = OwnedPath::try_from(format!("/{}", ticket_hash))?;
        let owner_path = OwnedPath::try_from(format!("/{}", owner.to_base58()))?;

        Ok(path::concat(
            &TICKET_TABLE_PATH,
            &path::concat(&ticket_hash_path, &owner_path)?,
        )?)
    }

    pub fn get_balance(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &impl Addressable,
        ticket_hash: &TicketHash,
    ) -> Result<Amount> {
        let path = Self::path(ticket_hash, owner)?;
        let result = tx.get::<Amount>(rt, path)?;
        match result {
            Some(balance) => Ok(*balance),
            None => Ok(0),
        }
    }

    /// Adds the given `amount` from the ticket balance of `owner`
    /// for the ticket `ticket_hash` and returns the account's new balance.
    /// Creates the account if it doesn't exist. Fails if the addition causes
    /// an overflow.
    pub fn add(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &impl Addressable,
        ticket_hash: &TicketHash,
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

    /// Subtracts the given `amount` from the ticket balance of `owner`
    /// for the ticket `ticket_hash` and returns the account's new balance.
    /// Fails if the account doesn't exist or the account has insufficient funds.
    pub fn sub(
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        owner: &impl Addressable,
        ticket_hash: &TicketHash,
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
    use crate::context::account::Address;

    use super::*;
    use jstz_core::kv::Transaction;
    use jstz_crypto::{
        hash::Hash, public_key_hash::PublicKeyHash,
        smart_function_hash::SmartFunctionHash,
    };
    use jstz_mock::host::JstzMockHost;
    use tezos_smart_rollup_mock::MockHost;

    fn user_address() -> Address {
        Address::User(
            PublicKeyHash::from_base58("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx")
                .expect("Could not parse pkh"),
        )
    }

    fn smart_function_address() -> Address {
        Address::SmartFunction(
            SmartFunctionHash::from_base58("KT1RycYvM4EVs6BAXWEsGXaAaRqiMP53KT4w")
                .expect("Could not parse smart function hash"),
        )
    }

    #[test]
    fn path_format() {
        let ticket_hash = jstz_mock::ticket_hash1();
        let owner = user_address();
        let result = TicketTable::path(&ticket_hash, &owner).unwrap();
        let expected = "/ticket_table/4db276d5f50bc2ad959b0f08bb34fbdf4fbe4bf95a689ffb9e922038430840d7/tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx";
        assert_eq!(expected, result.to_string());
    }

    #[test]
    fn add_tickets_succeeds() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();

        tx.begin();
        let owner = user_address();
        let ticket_hash = jstz_mock::ticket_hash1();
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
        let owner = user_address();
        let ticket_hash = jstz_mock::ticket_hash1();
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
        let owner = user_address();
        let ticket_hash = jstz_mock::ticket_hash1();
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
        let owner = user_address();
        let ticket_hash = jstz_mock::ticket_hash1();
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
        let owner = user_address();
        let ticket_hash = jstz_mock::ticket_hash1();

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

    #[test]
    fn smart_function_ticket_operations() {
        let mut host = JstzMockHost::default();
        let mut tx = Transaction::default();
        tx.begin();

        let owner = smart_function_address();
        let ticket_hash = jstz_mock::ticket_hash1();

        // Test initial balance is 0
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(0, balance);

        // Test adding tickets
        let amount = 100;
        TicketTable::add(host.rt(), &mut tx, &owner, &ticket_hash, amount).unwrap();
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(100, balance);

        // Test subtracting tickets
        let sub_amount = 30;
        TicketTable::sub(host.rt(), &mut tx, &owner, &ticket_hash, sub_amount).unwrap();
        let balance =
            TicketTable::get_balance(host.rt(), &mut tx, &owner, &ticket_hash).unwrap();
        assert_eq!(70, balance);
    }
}
