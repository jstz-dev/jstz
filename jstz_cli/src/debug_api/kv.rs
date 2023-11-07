use std::ops::Deref;

use boa_engine::{
    js_string, object::ObjectInitializer, Context, JsArgs, JsNativeError, JsObject,
    JsResult, JsString, JsValue, NativeFunction,
};
use jstz_api::{Kv, KvValue};
use jstz_core::{host_defined, kv::Transaction, runtime};

macro_rules! preamble {
    ($args:ident, $context:ident, $tx:ident) => {
        host_defined!($context, host_defined);
        let mut $tx = host_defined
            .get_mut::<Transaction>()
            .expect("Curent transaction undefined");
    };
}

macro_rules! set_value {
    ($args:ident, $value:ident, $id:tt) => {
        let $value = $args
            .get_or_undefined($id)
            .as_string()
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `String`")
            })
            .map(JsString::to_std_string_escaped)?;
    };
}

pub struct KvApi;

impl KvApi {
    fn get(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);
        set_value!(args, account, 0);
        set_value!(args, key, 1);

        let kv = Kv::new(account);

        let result = runtime::with_global_host(|rt| kv.get(rt.deref(), &mut tx, &key))?;

        match result {
            Some(value) => JsValue::from_json(&value.0, context),
            None => Ok(JsValue::null()),
        }
    }

    fn set(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);
        set_value!(args, account, 0);
        set_value!(args, key, 1);

        let value = KvValue(args.get_or_undefined(2).to_json(context)?);

        let kv = Kv::new(account);

        kv.set(&mut tx, &key, value)?;

        Ok(JsValue::undefined())
    }

    fn delete(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);
        set_value!(args, account, 0);
        set_value!(args, key, 1);

        let kv = Kv::new(account);

        runtime::with_global_host(|hrt| kv.delete(hrt.deref(), &mut tx, &key))?;

        Ok(JsValue::undefined())
    }

    fn has(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        preamble!(args, context, tx);
        set_value!(args, account, 0);
        set_value!(args, key, 1);

        let kv = Kv::new(account);

        let result = runtime::with_global_host(|rt| kv.has(rt.deref(), &mut tx, &key))?;

        Ok(result.into())
    }

    pub fn init(self, context: &mut boa_engine::Context<'_>) -> JsObject {
        let storage = ObjectInitializer::new(context)
            .function(NativeFunction::from_fn_ptr(Self::get), js_string!("get"), 2)
            .function(NativeFunction::from_fn_ptr(Self::set), js_string!("set"), 3)
            .function(
                NativeFunction::from_fn_ptr(Self::delete),
                js_string!("delete"),
                2,
            )
            .function(NativeFunction::from_fn_ptr(Self::has), js_string!("has"), 2)
            .build();

        storage
    }
}
