use std::ops::Deref;

use boa_engine::{
    object::ObjectInitializer, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, Trace};
use jstz_core::{host_defined, kv::Transaction, runtime};
use jstz_crypto::public_key_hash::PublicKeyHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup_host::path::{self, OwnedPath, RefPath};
use tezos_smart_rollup_host::runtime::Runtime;

use crate::{conversion::ToJs, error::Result};

#[derive(Debug, Trace, Finalize)]
struct Storage {
    prefix: String,
}

const STORAGE_PATH: RefPath = RefPath::assert_from(b"/jstz_storage");

// TODO: Figure out a more effective way of serializing values using json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StorageValue(serde_json::Value);

impl Into<String> for StorageValue {
    fn into(self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<String> for StorageValue {
    type Error = serde_json::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self(serde_json::from_str(&value)?))
    }
}

impl ToJs for &StorageValue {
    fn to_js(self, context: &mut Context) -> JsResult<JsValue> {
        JsValue::from_json(&self.0, context)
    }
}

impl Storage {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }

    fn key_path(&self, key: &str) -> jstz_core::Result<OwnedPath> {
        let key_path = OwnedPath::try_from(format!("/{}/{}", self.prefix, key))?;

        Ok(path::concat(&STORAGE_PATH, &key_path)?)
    }

    fn set(&self, tx: &mut Transaction, key: &str, value: StorageValue) -> Result<()> {
        Ok(tx.insert(self.key_path(key)?, value)?)
    }

    fn get<'a>(
        &self,
        rt: &impl Runtime,
        tx: &'a mut Transaction,
        key: &str,
    ) -> Result<Option<&'a StorageValue>> {
        Ok(tx.get::<StorageValue>(rt, self.key_path(key)?)?)
    }

    fn delete(&self, rt: &impl Runtime, tx: &mut Transaction, key: &str) -> Result<()> {
        Ok(tx.remove(rt, &self.key_path(key)?)?)
    }

    fn has(&self, rt: &impl Runtime, tx: &mut Transaction, key: &str) -> Result<bool> {
        Ok(tx.contains_key(rt, &self.key_path(key)?)?)
    }
}

macro_rules! preamble {
    ($this:ident, $args:ident, $context:ident, $key:ident, $tx:ident) => {
        host_defined!($context, host_defined);
        let mut $tx = host_defined.get_mut::<Transaction>().expect("");

        let $this =
            $this
                .as_object()
                .and_then(|obj| obj.downcast_mut::<Storage>())
                .ok_or_else(|| {
                    JsError::from_native(JsNativeError::typ().with_message(
                        "Failed to convert js value into rust type `Storage`",
                    ))
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

pub struct StorageApi {
    pub contract_address: PublicKeyHash,
}

impl StorageApi {
    const NAME: &'static str = "Storage";

    fn set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let value = StorageValue(args.get_or_undefined(1).to_json(context)?);

        this.set(&mut tx, &key, value)?;

        Ok(JsValue::undefined())
    }

    fn get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let result = runtime::with_global_host(|rt| this.get(rt.deref(), &mut tx, &key))?;

        result.to_js(context)
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        runtime::with_global_host(|rt| this.delete(rt.deref(), &mut tx, &key))?;

        Ok(JsValue::undefined())
    }

    fn has(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let result = runtime::with_global_host(|rt| this.has(rt.deref(), &mut tx, &key))?;

        Ok(result.into())
    }
}

impl jstz_core::realm::Api for StorageApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let storage = ObjectInitializer::with_native(
            Storage::new(self.contract_address.to_string()),
            context,
        )
        .function(NativeFunction::from_fn_ptr(Self::set), "set", 2)
        .function(NativeFunction::from_fn_ptr(Self::get), "get", 1)
        .function(NativeFunction::from_fn_ptr(Self::delete), "delete", 1)
        .function(NativeFunction::from_fn_ptr(Self::has), "has", 1)
        .build();

        context
            .register_global_property(Self::NAME, storage, Attribute::all())
            .expect("The storage object shouldn't exist yet");
    }
}
