use std::array::TryFromSliceError;

use boa_gc::{empty_trace, Finalize, Trace};
use derive_more::{Display, Error};
use hex::FromHexError;
use serde::{Deserialize, Serialize};
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
    Finalize,
)]
pub struct Blake2b([u8; 32]);

unsafe impl Trace for Blake2b {
    empty_trace!();
}

impl ToString for Blake2b {
    fn to_string(&self) -> String {
        hex::encode(self.0)
    }
}

impl<'a> From<&'a [u8]> for Blake2b {
    fn from(data: &'a [u8]) -> Self {
        let digest = tezos_crypto_rs::blake2b::digest_256(data);
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

    // Deserialises a hex encoded Blake2b hash string
    pub fn try_parse(hex_encode: String) -> Result<Self, Blake2bError> {
        let data = hex::decode(hex_encode).map_err(Blake2bError::DecodeError)?;
        let data: [u8; 32] = data
            .as_slice()
            .try_into()
            .map_err(Blake2bError::InvalidLength)?;
        Ok(Self(data))
    }
}

#[derive(Debug, Error, Display)]
pub enum Blake2bError {
    DecodeError(FromHexError),
    InvalidLength(TryFromSliceError),
}
