use std::ops::Deref;

use boa_engine::{
    js_string, object::ObjectInitializer, Context, JsArgs, JsObject, JsResult, JsValue,
    NativeFunction,
};
use jstz_core::runtime;
use jstz_proto::runtime::{Kv, KvValue};

pub struct KvApi;

impl KvApi {
    fn get(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let key: String = args.get_or_undefined(1).try_js_into(context)?;

        let kv = Kv::new(account);

        runtime::with_js_hrt_and_tx(|hrt, tx| -> JsResult<JsValue> {
            match kv.get(hrt.deref(), tx, &key)? {
                Some(value) => JsValue::from_json(&value.0, context),
                None => Ok(JsValue::null()),
            }
        })
    }

    fn set(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let key: String = args.get_or_undefined(1).try_js_into(context)?;

        let value = KvValue(args.get_or_undefined(2).to_json(context)?);

        let kv = Kv::new(account);

        runtime::with_js_tx(|tx| kv.set(tx, &key, value))?;

        Ok(JsValue::undefined())
    }

    fn delete(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let key: String = args.get_or_undefined(1).try_js_into(context)?;

        let kv = Kv::new(account);

        runtime::with_js_tx(|tx| kv.delete(tx, &key))?;

        Ok(JsValue::undefined())
    }

    fn has(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let account: String = args.get_or_undefined(0).try_js_into(context)?;
        let key: String = args.get_or_undefined(1).try_js_into(context)?;

        let kv = Kv::new(account);

        let result =
            runtime::with_js_hrt_and_tx(|hrt, tx| kv.has(hrt.deref(), tx, &key))?;

        Ok(result.into())
    }

    pub fn namespace(context: &mut boa_engine::Context) -> JsObject {
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
