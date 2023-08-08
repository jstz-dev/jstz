use super::{Address, ByteRep};
use serde::{Deserialize, Serialize};
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    creating_address: Address,
    code: String,
    nonce: u64,
}

impl Contract {
    pub fn create_address(&self) -> Address {
        let hash = keccak_hash::keccak(ByteRep::from_t(self).bytes().as_slice());
        format!("JsTz{hash:x}").try_into().unwrap()
    }
}
