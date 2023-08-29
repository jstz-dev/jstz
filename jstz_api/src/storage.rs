use std::ops::DerefMut;

use boa_engine::object::ObjectInitializer;
use boa_engine::{property::Attribute, Context, JsResult, JsValue, NativeFunction};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::{
    host::{self, Host},
    host_defined,
    kv::Transaction,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup_host::path::OwnedPath;
use tezos_smart_rollup_host::runtime::{Runtime, RuntimeError};

use crate::conversion::{ExpectString, FromJs, ToJs};

struct StoredPrefix(String);
unsafe impl Trace for StoredPrefix {
    empty_trace!();
}
impl Finalize for StoredPrefix {}
impl From<PublicKeyHash> for StoredPrefix {
    fn from(source: PublicKeyHash) -> Self {
        Self(source.to_string())
    }
}
macro_rules! setup_call {
    (this: $this:ident, context: $context:ident $(, host: $rt:ident)? $(, transaction: $tx:ident)?) => {
      host_defined!($context, host_defined);
      $(let mut $rt = host_defined.get_mut::<Host<H>>().expect("");)?
      $(let mut $tx = host_defined.get_mut::<Transaction>().expect("");)?
      let $this = $this.as_object()
            .and_then(|obj| obj.downcast_mut::<StoredPrefix>()).unwrap();

    };
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsStoreValue(Vec<u16>);
impl FromJs for JsStoreValue {
    fn from_js(this: &JsValue, context: &mut Context) -> JsResult<Self> {
        let string = this.to_string(context)?;
        let words = string.iter().map(|x| *x).collect();
        Ok(Self(words))
    }
}
impl ToJs for &JsStoreValue {
    fn to_js(self, _context: &mut Context) -> JsResult<JsValue> {
        Ok(JsValue::String(self.0.clone().into()))
    }
}

impl StoredPrefix {
    fn create_path(&self, name: &str) -> Result<OwnedPath, RuntimeError> {
        let prefix = &self.0;
        let path = format!("/{prefix}/{name}").to_string();
        path.try_into().map_err(|_| RuntimeError::PathNotFound)
    }

    fn write_value(
        &self,
        tx: &mut Transaction,
        name: &str,
        payload: JsStoreValue,
    ) -> Result<(), jstz_core::Error> {
        let path = self.create_path(name)?;
        tx.insert(path, payload)
    }
    fn read_value<'a>(
        &self,
        rt: &mut impl Runtime,
        tx: &'a mut Transaction,
        name: &str,
    ) -> Result<Option<&'a JsStoreValue>, jstz_core::Error> {
        let path = self.create_path(name)?;
        tx.get::<JsStoreValue>(rt, path)
    }
    fn remove_value(
        &self,
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        name: &str,
    ) -> Result<(), jstz_core::Error> {
        let path = self.create_path(name)?;
        tx.remove(rt, &path)
    }
    fn has_value(
        &self,
        rt: &mut impl Runtime,
        tx: &mut Transaction,
        name: &str,
    ) -> Result<bool, jstz_core::Error> {
        let path = self.create_path(name)?;
        tx.contains_key(rt, &path)
    }
}

pub struct StorageApi {
    pub contract_address: PublicKeyHash,
}

impl jstz_core::host::Api for StorageApi {
    fn init<H: Runtime + 'static>(self, context: &mut boa_engine::Context<'_>) {
        fn write_value<H: Runtime + 'static>(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            setup_call!(this: this, context: context, transaction: tx);
            let ExpectString(key) = FromJs::from_js_args(&args, 0, context)?;
            let value = JsStoreValue::from_js_args(&args, 1, context)?;

            let result: Result<(), crate::error::Error> = this
                .write_value(&mut tx, &key, value)
                .map_err(|err| err.into());
            result.to_js(context)
        }
        fn read_value<H: Runtime + 'static>(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            setup_call!(this: this, context: context, host: rt, transaction: tx);
            let ExpectString(key) = FromJs::from_js_args(&args, 0, context)?;
            let result: Result<_, crate::error::Error> = this
                .read_value(rt.deref_mut(), &mut tx, &key)
                .map_err(|err| err.into());
            result.to_js(context)
        }
        fn remove_value<H: Runtime + 'static>(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            setup_call!(this: this, context: context, host: rt, transaction: tx);
            let ExpectString(key) = FromJs::from_js_args(&args, 0, context)?;
            let result: Result<(), crate::error::Error> = this
                .remove_value(rt.deref_mut(), &mut tx, &key)
                .map_err(|err| err.into());
            result.to_js(context)
        }
        fn has_value<H: Runtime + 'static>(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            setup_call!(this: this, context: context, host: rt, transaction: tx);
            let ExpectString(key) = FromJs::from_js_args(&args, 0, context)?;
            let result: Result<bool, crate::error::Error> = this
                .has_value(rt.deref_mut(), &mut tx, &key)
                .map_err(|err| err.into());
            result.to_js(context)
        }
        let prefix: StoredPrefix = self.contract_address.into();
        let storage = ObjectInitializer::with_native(prefix, context)
            .function(NativeFunction::from_fn_ptr(write_value::<H>), "setItem", 2)
            .function(NativeFunction::from_fn_ptr(read_value::<H>), "getItem", 1)
            .function(
                NativeFunction::from_fn_ptr(remove_value::<H>),
                "removeItem",
                1,
            )
            .function(NativeFunction::from_fn_ptr(has_value::<H>), "hasItem", 1)
            .build();
        context
            .register_global_property("storage", storage, Attribute::all())
            .expect("The storage object shouldn't exist yet");
    }
}
