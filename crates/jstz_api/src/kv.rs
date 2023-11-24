use std::ops::{Deref, DerefMut};

use boa_engine::{
    js_string, object::ObjectInitializer, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    host::HostRuntime, host_defined, kv::Transaction, realm::HostDefined, runtime, Result,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::storage::path::{self, OwnedPath, RefPath};

#[derive(Debug, Trace, Finalize)]
pub struct Kv {
    prefix: String,
}

const KV_PATH: RefPath = RefPath::assert_from(b"/jstz_kv");

// TODO: Figure out a more effective way of serializing values using json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KvValue(pub serde_json::Value);

impl From<KvValue> for String {
    fn from(val: KvValue) -> Self {
        val.0.to_string()
    }
}

impl TryFrom<String> for KvValue {
    type Error = serde_json::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self(serde_json::from_str(&value)?))
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
        &'a self,
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction<'static>,
        key: &str,
    ) -> Result<Option<&'a KvValue>> {
        tx.get::<KvValue>(hrt, self.key_path(key)?)
    }

    pub fn delete<'a>(
        &'a self,
        hrt: &impl HostRuntime,
        tx: &'a mut Transaction<'static>,
        key: &str,
    ) -> Result<()> {
        tx.remove(hrt, &self.key_path(key)?)
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
    ($this:ident, $args:ident, $context:ident, $key:ident, $tx:ident) => {
        host_defined!($context, host_defined);
        let mut $tx = host_defined
            .get_mut::<Transaction<'static>>()
            .expect("Curent transaction undefined");

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

macro_rules! preamble_static {
    ($this:ident, $args:ident, $context:ident, $key:ident, $tx:ident) => {
        let host_defined_binding = $context
            .global_object()
            .get(js_string!(HostDefined::NAME), $context)
            .expect(&format!("{:?} should be defined", HostDefined::NAME));

        let mut host_defined: GcRefMut<'_, _, HostDefined> = host_defined_binding
            .as_object()
            .expect("Failed to convert js value to a js object")
            .downcast_mut::<HostDefined>()
            .expect("Failed to convert js object to rust type `HostDefined`");

        let mut $tx: GcRefMut<'_, _, Transaction<'static>> =
            HostDefined::get_mut::<Transaction<'static>>(host_defined.deref_mut())
                .expect("Curent transaction undefined");

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
    pub contract_address: PublicKeyHash,
}

impl KvApi {
    const NAME: &'static str = "Kv";

    fn set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let value = KvValue(args.get_or_undefined(1).to_json(context)?);

        this.set(&mut tx, &key, value)?;

        Ok(JsValue::undefined())
    }

    fn get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble_static!(this, args, context, key, tx);

        let result =
            runtime::with_global_host(|rt| this.get(rt.deref(), tx.deref_mut(), &key))?;

        match result {
            Some(value) => JsValue::from_json(&value.0, context),
            None => Ok(JsValue::null()),
        }
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble_static!(this, args, context, key, tx);

        runtime::with_global_host(|hrt| this.delete(hrt.deref(), tx.deref_mut(), &key))?;

        Ok(JsValue::undefined())
    }

    fn has(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        preamble!(this, args, context, key, tx);

        let result =
            runtime::with_global_host(|hrt| this.has(hrt.deref(), &mut tx, &key))?;

        Ok(result.into())
    }
}

impl jstz_core::Api for KvApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let storage = ObjectInitializer::with_native(
            Kv::new(self.contract_address.to_string()),
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
mod test {
    use super::*;
    use jstz_proto::context::account::Account;
    use tezos_smart_rollup_mock::MockHost;

    #[test]
    fn test_nested_transactions() -> Result<()> {
        let hrt = &mut MockHost::default();

        let mut tx = Transaction::new();

        let pkh = PublicKeyHash::from_base58("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q")
            .expect("Could not parse pkh");

        // Act
        let amt = {
            // This mutable borrow ends at the end of this block
            Account::balance(hrt, &mut tx, &pkh).expect("Could not get balance")
        };

        {
            {
                let mut child_tx = tx.begin();
            }
            /*{

                {
                    let mut grandchild_tx = child_tx.begin();
                    grandchild_tx
                        .commit::<Account>(hrt)
                        .expect("Could not commit tx");
                }
                child_tx
                    .commit::<Account>(hrt)
                    .expect("Could not commit tx");
            }*/
            {
                tx.commit::<Account>(hrt).expect("Could not commit tx");
            }
        }

        // Assert
        assert_eq!(amt, 0);

        Ok(())
    }
}
