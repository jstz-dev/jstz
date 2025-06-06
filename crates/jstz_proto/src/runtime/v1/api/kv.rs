use std::ops::Deref;

use bincode::error::{DecodeError, EncodeError};
use bincode::{de::Decoder, enc::Encoder, Decode, Encode};
use boa_engine::{
    js_string, object::ObjectInitializer, property::Attribute, Context, JsArgs, JsData,
    JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, Trace};
use jstz_core::kv::transaction::Guarded;
use jstz_core::{host::HostRuntime, kv::Transaction, runtime, Result};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup_host::path::{self, OwnedPath, RefPath};
use utoipa::ToSchema;

#[derive(Debug, Trace, Finalize, JsData)]
pub struct Kv {
    prefix: String,
}

const KV_PATH: RefPath = RefPath::assert_from(b"/jstz_kv");

// TODO: Figure out a more effective way of serializing values using json
/// A value stored in the Key-Value store. Always valid JSON.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(value_type = Value)]
pub struct KvValue(pub serde_json::Value);

impl Decode for KvValue {
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

    fn key_path(&self, key: &str) -> jstz_core::Result<OwnedPath> {
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

macro_rules! preamble {
    ($this:ident, $args:ident, $key:ident) => {
        let $this = $this
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Kv>())
            .ok_or_else(|| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to convert js value into rust type `Kv`"),
                )
            })?;

        let $key = $args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `String`")
            })
            .map(JsString::to_std_string_escaped)?;
    };
}

pub struct KvApi {
    pub address: SmartFunctionHash,
}

impl KvApi {
    const NAME: &'static str = "Kv";

    fn set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, key);

        let value = KvValue(args.get_or_undefined(1).to_json(context)?);

        runtime::with_js_tx(|tx| this.set(tx, &key, value))?;

        Ok(JsValue::undefined())
    }

    fn get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, key);

        runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<JsValue> {
            match this.get(hrt.deref(), tx, &key)? {
                Some(value) => JsValue::from_json(&value.0, context),
                None => Ok(JsValue::null()),
            }
        })
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(this, args, key);

        runtime::with_js_tx(|tx| this.delete(tx, &key))?;

        Ok(JsValue::undefined())
    }

    fn has(
        this: &JsValue,
        args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(this, args, key);

        let result = runtime::with_js_hrt(|hrt| {
            runtime::with_js_tx(|tx| this.has(hrt.deref(), tx, &key))
        })?;

        Ok(result.into())
    }
}

impl jstz_core::Api for KvApi {
    fn init(self, context: &mut Context) {
        let storage = ObjectInitializer::with_native_data(
            Kv::new(self.address.to_string()),
            context,
        )
        .function(NativeFunction::from_fn_ptr(Self::set), js_string!("set"), 2)
        .function(NativeFunction::from_fn_ptr(Self::get), js_string!("get"), 1)
        .function(
            NativeFunction::from_fn_ptr(Self::delete),
            js_string!("delete"),
            1,
        )
        .function(NativeFunction::from_fn_ptr(Self::has), js_string!("has"), 1)
        .build();

        context
            .register_global_property(js_string!(Self::NAME), storage, Attribute::all())
            .expect("The storage object shouldn't exist yet");
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
