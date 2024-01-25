#[cfg(test)]
mod test {
    use std::ops::Deref;
    use std::{cell::RefCell, rc::Rc};

    use jstz_core::{error::Result, host::HostRuntime, kv, kv::transaction::Transaction};
    use jstz_crypto::keypair_from_passphrase;
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_proto::context::account::Account;
    use tezos_smart_rollup_mock::MockHost;

    fn get_random_public_key_hash(passphrase: &str) -> PublicKeyHash {
        let (_, pk) =
            keypair_from_passphrase(passphrase).expect("Failed to generate keypair");
        return PublicKeyHash::try_from(&pk)
            .expect("Failed to generate public key hash.");
    }

    fn get_account_balance_from_storage(
        hrt: &impl HostRuntime,
        pkh: &PublicKeyHash,
    ) -> u64 {
        let account = match kv::Storage::get::<Account>(
            hrt,
            &Account::path(&pkh).expect("Could not get path"),
        )
        .expect("Could not find the account")
        {
            Some(account) => account,
            None => panic!("Account not found"),
        };

        account.amount
    }

    fn verify_account_balance(
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        pkh: &PublicKeyHash,
        expected: u64,
    ) {
        let amt = Account::balance(hrt, tx, &pkh).expect("Could not get balance");

        assert_eq!(amt, expected);
    }

    fn commit_transaction_mock(hrt: &mut MockHost, tx: &Rc<RefCell<Transaction>>) {
        tx.deref()
            .borrow_mut()
            .commit::<Account>(hrt)
            .expect("Could not commit tx");
    }

    #[test]
    fn test_nested_transactions() -> Result<()> {
        let hrt = &mut MockHost::default();
        let tx = Rc::new(RefCell::new(Transaction::new()));
        let pkh1 = get_random_public_key_hash("passphrase1");
        let pkh2 = get_random_public_key_hash("passphrase2");

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 0);
        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh2, 0);

        let child_tx = Transaction::begin(Rc::clone(&tx));

        let _ = Account::deposit(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 0);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);
        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh2, 0);

        let grandchild_tx = Transaction::begin(Rc::clone(&child_tx));

        verify_account_balance(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh2, 25);

        let _ = Account::deposit(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut grandchild_tx.deref().borrow_mut(), &pkh1, 57);

        commit_transaction_mock(hrt, &grandchild_tx);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh2, 25);

        let _ = Account::deposit(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut child_tx.deref().borrow_mut(), &pkh1, 2 * 57);

        commit_transaction_mock(hrt, &child_tx);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 2 * 57);

        let _ = Account::deposit(hrt, &mut tx.deref().borrow_mut(), &pkh1, 57);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 3 * 57);

        commit_transaction_mock(hrt, &tx);

        verify_account_balance(hrt, &mut tx.deref().borrow_mut(), &pkh1, 3 * 57);

        assert_eq!(get_account_balance_from_storage(hrt, &pkh1), 3 * 57);

        assert_eq!(get_account_balance_from_storage(hrt, &pkh2), 25);

        Ok(())
    }
}
