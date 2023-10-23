use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::accounts::account::Account;

// Represents a collection of accounts
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AccountList {
    accounts: HashMap<String, Account>,
}

impl AccountList {
    /*
    pub fn new() -> Self {
        AccountList {
            accounts: HashMap::new(),
        }
    }
    */

    pub fn upsert(&mut self, account: Account) {
        self.accounts.insert(account.get_alias().clone(), account);
    }

    /*
    pub fn remove(&mut self, alias: &String) -> Option<Account> {
        self.accounts.remove(alias)
    }

    pub fn get(&self, alias: &String) -> Option<&Account> {
        self.accounts.get(alias)
    }

    pub fn get_all(&self) -> &HashMap<String, Account> {
        &self.accounts
    }
    */
}
