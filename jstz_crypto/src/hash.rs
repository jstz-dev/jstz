use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Blake2b([u8; 32]);

impl ToString for Blake2b {
    fn to_string(&self) -> String {
        hex::encode(self.0)
    }
}

impl<'a> From<&'a [u8]> for Blake2b {
    fn from(data: &'a [u8]) -> Self {
        let digest = tezos_crypto_rs::blake2b::digest_256(data).unwrap();
        Self(digest.try_into().unwrap())
    }
}

impl<'a> From<&'a Vec<u8>> for Blake2b {
    fn from(data: &'a Vec<u8>) -> Self {
        let data = data.as_slice();
        Self::from(data)
    }
}

impl AsRef<[u8]> for Blake2b {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Blake2b {
    pub fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}
