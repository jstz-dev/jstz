use std::{
    array::TryFromSliceError,
    fmt::{self, Display},
    str::FromStr,
};

pub use crate::error::{Error, Result};
use core::hash::Hash as CoreHash;
use std::fmt::Debug;

use boa_gc::{empty_trace, Finalize, Trace};
use derive_more::{Display, Error};
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    CoreHash,
    Serialize,
    Deserialize,
    Default,
    Finalize,
    ToSchema,
)]
pub struct Blake2b([u8; 32]);

unsafe impl Trace for Blake2b {
    empty_trace!();
}

impl Display for Blake2b {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
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
    pub fn try_parse(hex_encode: String) -> core::result::Result<Self, Blake2bError> {
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

pub trait Hash<'a>:
    Sized
    + Debug
    + Clone
    + PartialEq
    + Eq
    + Serialize
    + Deserialize<'a>
    + Finalize
    // TODO: Add back when renamed
    // https://linear.app/tezos/issue/JSTZ-253/remove-old-accountrs
    // ToSchema,
    + Trace
    + FromStr
    + Display
{
    fn to_base58(&self) -> String;

    fn from_base58(data: &str) -> Result<Self>;

    fn as_bytes(&self) -> &[u8];

    fn digest(data: &[u8]) -> Result<Self>;
}
