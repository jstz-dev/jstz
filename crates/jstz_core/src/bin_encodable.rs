use bincode::{
    config::{Configuration, Fixint, Limit, LittleEndian},
    Decode, Encode,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::{Error, Result},
    reveal_data::MAX_REVEAL_SIZE,
};

// FixintEncoding is used for predictable, fixed-width integer encoding, which makes decoding
// more strict and less ambiguous compared to VarintEncoding.
// The decode limit (MAX_REVEAL_SIZE) is critical for safety — it prevents unbounded memory
// allocation on malformed input, mitigating potential denial-of-service (DoS) risks.
const BINCODE_CONFIGURATION: Configuration<LittleEndian, Fixint, Limit<MAX_REVEAL_SIZE>> =
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_limit();

/// Trait for types that can be encoded to and decoded from binary format
pub trait BinEncodable {
    fn encode(&self) -> Result<Vec<u8>>;
    fn decode(bytes: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

/// Default implementation for types that can be encoded to and decoded from binary format
impl<T: Encode + Decode> BinEncodable for T {
    fn encode(&self) -> Result<Vec<u8>> {
        bincode::encode_to_vec(self, BINCODE_CONFIGURATION).map_err(|err| {
            Error::SerializationError {
                description: format!("{err}"),
            }
        })
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let (value, _) = bincode::decode_from_slice(bytes, BINCODE_CONFIGURATION)
            .map_err(|err| Error::SerializationError {
                description: format!("{err}"),
            })?;
        Ok(value)
    }
}

pub fn serde_encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(value, BINCODE_CONFIGURATION).map_err(|err| {
        Error::SerializationError {
            description: format!("failed to serde encode: {err}"),
        }
    })
}

pub fn serde_decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let (value, _) = bincode::serde::decode_from_slice(bytes, BINCODE_CONFIGURATION)
        .map_err(|err| Error::SerializationError {
            description: format!("failed to serde decode: {err}"),
        })?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Encode, Decode)]
    struct TestData {
        field1: String,
        field2: i32,
    }

    #[test]
    fn test_binencodable_roundtrip() {
        let original = TestData {
            field1: "test".to_string(),
            field2: 42,
        };

        // Test encode
        let encoded = BinEncodable::encode(&original).unwrap();
        assert!(!encoded.is_empty());

        // Test decode
        let decoded = BinEncodable::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_binencodable_invalid_data() {
        // Try to decode invalid bytes
        let invalid_bytes = vec![1, 2, 3];
        let result = <TestData as BinEncodable>::decode(&invalid_bytes);
        assert!(result.is_err());

        // Verify error type
        match result {
            Err(Error::SerializationError { description: _ }) => (),
            _ => panic!("Expected SerializationError"),
        }
    }
    #[test]
    fn test_decode_without_limit_triggers_massive_allocation() {
        let mut malicious = Vec::new();

        // Craft a malicious payload with an absurdly large string length: 50 GB.
        let large_len = 50_000_000_000u64;
        malicious.extend_from_slice(&large_len.to_le_bytes()); // field1: String length = 50 GB
        malicious.extend_from_slice(&42i32.to_le_bytes());

        // Without a decode limit, this call may hang indefinitely or crash the process.
        let result = <TestData as BinEncodable>::decode(&malicious);
        assert!(result.is_err_and(|e| e.to_string().contains("LimitExceeded")));
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SerdeTestData {
        field1: String,
        field2: i32,
        field3: Vec<u8>,
    }

    #[test]
    fn test_serde_encode_decode_roundtrip() {
        let original = SerdeTestData {
            field1: "test".to_string(),
            field2: 42,
            field3: vec![1, 2, 3, 4, 5],
        };

        // Test encode
        let encoded = serde_encode(&original).unwrap();
        assert!(!encoded.is_empty());

        // Test decode
        let decoded: SerdeTestData = serde_decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_serde_encode_decode_invalid_data() {
        // Try to decode invalid bytes
        let invalid_bytes = vec![1, 2, 3];
        let result: std::result::Result<SerdeTestData, _> = serde_decode(&invalid_bytes);
        assert!(result.is_err());

        // Verify error type
        match result {
            Err(Error::SerializationError { description: _ }) => (),
            _ => panic!("Expected SerializationError"),
        }
    }
}
