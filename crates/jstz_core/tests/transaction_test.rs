#[cfg(test)]
mod test {

    use jstz_core::{error::Result, host::HostRuntime, kv, kv::transaction::Transaction};
    use jstz_crypto::keypair_from_passphrase;
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_proto::context::account::Account;
    use tezos_smart_rollup_mock::MockHost;

    fn get_random_public_key_hash(passphrase: &str) -> PublicKeyHash {
        let (_, pk) =
            keypair_from_passphrase(passphrase).expect("Failed to generate keypair");
        PublicKeyHash::try_from(&pk).expect("Failed to generate public key hash.")
    }

    fn get_account_balance_from_storage(
        hrt: &impl HostRuntime,
        pkh: &PublicKeyHash,
    ) -> u64 {
        let account = match kv::Storage::get::<Account>(
            hrt,
            &Account::path(pkh).expect("Could not get path"),
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
        let amt = Account::balance(hrt, tx, pkh).expect("Could not get balance");

        assert_eq!(amt, expected);
    }

    #[test]
    fn test_nested_transactions() -> Result<()> {
        let hrt = &mut MockHost::default();

        let mut tx = Transaction::default();

        // Transaction (tx0)

        tx.begin();

        let pkh1 = get_random_public_key_hash("passphrase1");
        let pkh2 = get_random_public_key_hash("passphrase2");

        verify_account_balance(hrt, &mut tx, &pkh1, 0);
        verify_account_balance(hrt, &mut tx, &pkh2, 0);

        // Transaction (tx1)

        tx.begin();

        let _ = Account::add_balance(hrt, &mut tx, &pkh2, 25);

        verify_account_balance(hrt, &mut tx, &pkh1, 0);
        verify_account_balance(hrt, &mut tx, &pkh2, 25);

        // Transaction (tx2)

        tx.begin();

        verify_account_balance(hrt, &mut tx, &pkh2, 25);

        let _ = Account::add_balance(hrt, &mut tx, &pkh1, 57);

        verify_account_balance(hrt, &mut tx, &pkh1, 57);

        tx.commit(hrt).expect("Could not commit tx");

        // Transaction (tx1)

        verify_account_balance(hrt, &mut tx, &pkh2, 25);

        let _ = Account::add_balance(hrt, &mut tx, &pkh1, 57);

        verify_account_balance(hrt, &mut tx, &pkh1, 2 * 57);

        tx.commit(hrt).expect("Could not commit tx");

        // Transaction (tx0)

        verify_account_balance(hrt, &mut tx, &pkh1, 2 * 57);

        let _ = Account::add_balance(hrt, &mut tx, &pkh1, 57);

        verify_account_balance(hrt, &mut tx, &pkh1, 3 * 57);

        tx.commit(hrt).expect("Could not commit tx");

        // Check storage

        assert_eq!(get_account_balance_from_storage(hrt, &pkh1), 3 * 57);

        assert_eq!(get_account_balance_from_storage(hrt, &pkh2), 25);

        Ok(())
    }
}
