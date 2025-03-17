use bincode::{
    config::{Configuration, Fixint, LittleEndian},
    Decode, Encode,
};

use crate::error::{Error, Result};

const BINCODE_CONFIGURATION: Configuration<LittleEndian, Fixint> =
    bincode::config::legacy();

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
