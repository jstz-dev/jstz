//! Temporary copy of KV from `../jstz_proto/api/kv.rs`
//!
//! The KV API is a runtime API and rightfully should belong in this crate. KV
//! behaviour itself does not depend on the protocol. Instead, it depends on
//! HostRuntime and Transaction which are mechanisms for interacting with the
//! kernel host, exposed through `jstz_core`. In the long run, we should deprecate
//! the KV API in `jstz_proto`.

use bincode::error::{DecodeError, EncodeError};
use bincode::{de::Decoder, enc::Encoder, Decode, Encode};
use jstz_core::host::HostRuntime;
use jstz_core::kv::transaction::Guarded;
use jstz_core::kv::Transaction;
use jstz_core::Result;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};
use utoipa::ToSchema;

#[derive(Debug)]
pub struct Kv {
    prefix: String,
}

const KV_PATH: RefPath = RefPath::assert_from(b"/jstz_kv");

// TODO: Figure out a more effective way of serializing values using json
/// A value stored in the Key-Value store. Always valid JSON.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(value_type = Value)]
pub struct KvValue(pub serde_json::Value);

impl Decode<()> for KvValue {
    fn decode<D: Decoder>(decoder: &mut D) -> std::result::Result<KvValue, DecodeError> {
        let bytes: Vec<u8> = Decode::decode(decoder)?;
        let value = serde_json::from_slice(&bytes).map_err(|e| {
            DecodeError::OtherString(format!("error deserializing kv value: {e}"))
        })?;
        Ok(Self(value))
    }
}

impl Encode for KvValue {
    fn encode<E: Encoder>(
        &self,
        encoder: &mut E,
    ) -> std::result::Result<(), EncodeError> {
        let bytes = serde_json::to_vec(&self.0).map_err(|e| {
            EncodeError::OtherString(format!("error serializing kv value: {e}"))
        })?;
        Encode::encode(&bytes, encoder)
    }
}

impl Kv {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }

    fn key_path(&self, key: &str) -> Result<OwnedPath> {
        let key_path = OwnedPath::try_from(format!("/{}/{}", self.prefix, key))?;
        Ok(path::concat(&KV_PATH, &key_path)?)
    }

    pub fn set(&self, tx: &mut Transaction, key: &str, value: KvValue) -> Result<()> {
        tx.insert(self.key_path(key)?, value)
    }

    pub fn get<'a>(
        &self,
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction,
        key: &str,
    ) -> Result<Option<Guarded<'a, KvValue>>> {
        tx.get::<KvValue>(hrt, self.key_path(key)?)
    }

    pub fn delete(&self, tx: &mut Transaction, key: &str) -> Result<()> {
        tx.remove(self.key_path(key)?)
    }

    pub fn has(
        &self,
        hrt: &impl HostRuntime,
        tx: &mut Transaction,
        key: &str,
    ) -> Result<bool> {
        tx.contains_key(hrt, &self.key_path(key)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use jstz_core::BinEncodable;
    use serde_json::json;

    #[test]
    fn test_kv_value_roundtrip() {
        let test_cases = vec![
            // Null
            KvValue(json!(null)),
            // Boolean
            KvValue(json!(true)),
            KvValue(json!(false)),
            // Numbers
            KvValue(json!(42)),
            KvValue(json!(-17.5)),
            KvValue(json!(0)),
            // String
            KvValue(json!("hello world")),
            KvValue(json!("")),
            // Array
            KvValue(json!([])),
            KvValue(json!([1, 2, 3])),
            KvValue(json!(["a", "b", null, true, 1.5])),
            // Object
            KvValue(json!({})),
            KvValue(json!({
                "string": "value",
                "number": 42,
                "bool": true,
                "null": null,
                "array": [1, 2, 3],
                "nested": {
                    "a": "b",
                    "c": [true, null]
                }
            })),
        ];

        for value in test_cases {
            let bytes =
                <KvValue as BinEncodable>::encode(&value).expect("Failed to encode");
            let decoded =
                <KvValue as BinEncodable>::decode(&bytes).expect("Failed to decode");
            assert_eq!(
                value.0, decoded.0,
                "Value did not match after roundtrip: {:?}",
                value.0
            );
        }
    }

    #[test]
    fn test_kv_value_decode_error() {
        let invalid_bytes = b"invalid";
        let result = <KvValue as BinEncodable>::decode(invalid_bytes);
        assert!(result.is_err());
    }
}
