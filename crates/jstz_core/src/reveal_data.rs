use crate::{host::HostRuntime, BinEncodable};
use bincode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tezos_smart_rollup::{
    core_unsafe::PREIMAGE_HASH_SIZE,
    dac::{self, PreimageHash, PreimageHashError, V0SliceContentPage},
};

pub struct RevealData<'a, H: HostRuntime> {
    host: &'a mut H,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageHash(pub [u8; PREIMAGE_HASH_SIZE]);

impl Serialize for PageHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for PageHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as bytes first
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        // Then use bincode's decode
        Ok(PageHash(bytes.try_into().unwrap()))
    }
}

impl Default for PageHash {
    fn default() -> Self {
        PageHash([0; PREIMAGE_HASH_SIZE])
    }
}

impl AsRef<[u8; PREIMAGE_HASH_SIZE]> for PageHash {
    fn as_ref(&self) -> &[u8; PREIMAGE_HASH_SIZE] {
        &self.0
    }
}

impl From<[u8; PREIMAGE_HASH_SIZE]> for PageHash {
    fn from(hash: [u8; PREIMAGE_HASH_SIZE]) -> Self {
        PageHash(hash)
    }
}

impl<'a, H: HostRuntime> RevealData<'a, H> {
    pub fn new(host: &'a mut H) -> Self {
        RevealData { host }
    }

    pub fn reveal<F>(
        &mut self,
        root_hash: &PageHash,
        save_content: &mut F,
    ) -> Result<(), &'static str>
    where
        F: FnMut(&mut H, V0SliceContentPage) -> Result<(), &'static str>,
    {
        let mut buffer = [0u8; 4096]; // Buffer for reading preimages
        dac::reveal_loop(
            self.host,
            0,
            root_hash.as_ref(),
            &mut buffer,
            3,
            save_content,
        )
    }

    pub fn reveal_and_decode<T>(
        &mut self,
        root_hash: &PageHash,
    ) -> Result<T, &'static str>
    where
        T: BinEncodable,
    {
        let content = [0u8; 4096];
        /// use reveal function to fill content and decode it into T
        // self.reveal(root_hash, |_, page| {
        //     content.copy_from_slice(page.as_ref());
        //     Ok(())
        // })?;
        T::decode(&mut &content[..]).map_err(|_| "Failed to decode")
    }

    pub fn encode_into_preimage_pages<T, F>(
        value: &T,
        save_content: &mut F,
    ) -> Result<PreimageHash, PreimageHashError>
    where
        T: BinEncodable,
        F: FnMut(PreimageHash, Vec<u8>),
    {
        let v = T::encode(value).unwrap();
        dac::prepare_preimages(&v, save_content)
    }
}
