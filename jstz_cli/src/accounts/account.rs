use serde::{Deserialize, Serialize};

// Represents an individual account
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    alias: String,
    address: String,
}

impl Account {
    pub fn new(alias: String, address: String) -> Self {
        Account { alias, address }
    }

    pub fn get_alias(&self) -> &String {
        &self.alias
    }

    /*
    pub fn get_address(&self) -> &String {
        &self.address
    }

    pub fn set_alias(&mut self, alias: String) {
        self.alias = alias;
    }

    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }
    */
}
