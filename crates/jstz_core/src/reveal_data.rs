use crate::error::Result;
use crate::{host::HostRuntime, BinEncodable};
use derive_more::From;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::fmt;
use tezos_smart_rollup::{
    core_unsafe::PREIMAGE_HASH_SIZE,
    dac::{
        self, PreimageHash as DacPreimageHash, PreimageHashError, V0SliceContentPage,
        MAX_PAGE_SIZE,
    },
};
use thiserror::Error;

// Maximum number of DAC levels to support, can reveal up to 59MB of data.
const MAX_DAC_LEVELS: usize = 3;
// Support `MAX_DAC_LEVELS` levels of hashes pages, + the bottom layer of content.
const MAX_REVEAL_BUFFER_SIZE: usize = MAX_PAGE_SIZE * (MAX_DAC_LEVELS + 1);
/// maximum size of the reveal data in bytes (10MB)
pub const MAX_REVEAL_SIZE: usize = 10 * 1024 * 1024;

/// A 33-byte hash corresponding to a preimage
type RawPreimageHash = [u8; PREIMAGE_HASH_SIZE];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PreimageHash(#[serde(with = "BigArray")] pub RawPreimageHash);

impl Default for PreimageHash {
    fn default() -> Self {
        PreimageHash([0; PREIMAGE_HASH_SIZE])
    }
}

impl AsRef<RawPreimageHash> for PreimageHash {
    fn as_ref(&self) -> &[u8; PREIMAGE_HASH_SIZE] {
        &self.0
    }
}

impl From<RawPreimageHash> for PreimageHash {
    fn from(hash: [u8; PREIMAGE_HASH_SIZE]) -> Self {
        PreimageHash(hash)
    }
}

impl From<DacPreimageHash> for PreimageHash {
    fn from(hash: DacPreimageHash) -> Self {
        PreimageHash(*hash.as_ref())
    }
}

impl From<PreimageHash> for DacPreimageHash {
    fn from(hash: PreimageHash) -> Self {
        DacPreimageHash::from(hash.as_ref())
    }
}

impl fmt::Display for PreimageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.as_ref()))
    }
}

#[derive(Debug, Error, From)]
pub enum Error {
    #[error("Errors encountered while revealing data: {description}")]
    RevealDataError { description: String },
    #[error("Errors encountered while constructing a preimage hash.")]
    PreimageHashConstructionError(PreimageHashError),
    #[error("Reveal data size exceeds the maximum limit.")]
    RevealDataSizeExceedsMaximumLimit,
}

// TODO: optimize the api and performance
// https://linear.app/tezos/issue/JSTZ-359/optimize-reveal-data
pub struct RevealData;

impl RevealData {
    fn reveal<H, F>(
        hrt: &mut H,
        root_hash: &PreimageHash,
        save_content: &mut F,
    ) -> Result<()>
    where
        H: HostRuntime,
        F: FnMut(&mut H, V0SliceContentPage) -> std::result::Result<(), &'static str>,
    {
        let mut reveal_buffer = [0; MAX_REVEAL_BUFFER_SIZE];
        dac::reveal_loop(
            hrt,
            0,
            root_hash.as_ref(),
            &mut reveal_buffer,
            MAX_DAC_LEVELS,
            save_content,
        )
        .map_err(|e| e.to_owned().into())
    }

    /// Reveal the data and decode it into the given type.
    pub fn reveal_and_decode<H, T>(hrt: &mut H, root_hash: &PreimageHash) -> Result<T>
    where
        H: HostRuntime,
        T: BinEncodable,
    {
        // TODO: include the size of the data in the operation to avoid the allocation of a large buffer
        // https://linear.app/tezos/issue/JSTZ-359/optimize-reveal-data
        let mut content = Vec::with_capacity(10 * MAX_PAGE_SIZE);
        Self::reveal(
            hrt,
            root_hash,
            &mut |_: &mut H, page: V0SliceContentPage| {
                content.extend_from_slice(page.as_ref());
                Ok(())
            },
        )?;
        T::decode(&content[..])
    }

    /// Encode the data, prepare the preimages and return the root preimage hash.
    pub fn encode_and_prepare_preimages<T, F>(
        value: &T,
        mut handle: F,
    ) -> Result<PreimageHash>
    where
        T: BinEncodable,
        F: FnMut(PreimageHash, Vec<u8>),
    {
        let encoded = T::encode(value)?;
        if encoded.len() > MAX_REVEAL_SIZE {
            return Err(Error::RevealDataSizeExceedsMaximumLimit.into());
        }
        let hash =
            dac::prepare_preimages(&encoded, |hash: DacPreimageHash, data: Vec<u8>| {
                let hash = PreimageHash::from(hash);
                handle(hash, data);
            })
            .map_err(Error::PreimageHashConstructionError)?;
        Ok(hash.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::BinEncodable;
    use bincode::{Decode, Encode};
    use tezos_smart_rollup_mock::MockHost;

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    pub struct TestData(pub Vec<u8>);

    #[test]
    fn encode_and_prepare_preimages_fails_if_size_exceeds() {
        let preimage = TestData(vec![0; MAX_REVEAL_SIZE + 1]);
        let err = RevealData::encode_and_prepare_preimages(&preimage, |_, _| {})
            .expect_err("should fail");
        assert!(matches!(err, Error::RevealDataError { .. }));
    }

    #[test]
    fn test_encode_and_decode_with_rdc() {
        let data = TestData(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        encode_and_decode_data_with_rdc(data);
    }

    #[test]
    fn test_encode_and_decode_with_rdc_large_data() {
        let sample: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let large_data = vec![sample.clone(); MAX_REVEAL_SIZE / sample.len()]
            .into_iter()
            .flatten()
            .collect::<Vec<u8>>();
        let large_data = TestData(large_data);
        encode_and_decode_data_with_rdc(large_data);
    }

    fn encode_and_decode_data_with_rdc<T>(data: T)
    where
        T: BinEncodable + Clone + PartialEq + Eq + std::fmt::Debug,
    {
        let mut host = MockHost::default();
        let preimage_hash = RevealData::encode_and_prepare_preimages(&data, |_, page| {
            host.set_preimage(page);
        })
        .expect("should prepare preimages");

        let decoded =
            RevealData::reveal_and_decode::<_, T>(&mut host, &preimage_hash).unwrap();
        assert_eq!(decoded, data);
    }
}
