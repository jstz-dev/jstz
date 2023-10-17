use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsString, JsValue};

///   This is an example/test implementation of the refactored api
/// Very minimal implementation of a few core features
fn string_from_arg(args: &[JsValue], index: usize) -> JsResult<String> {
    Ok(args
        .get_or_undefined(index)
        .as_string()
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message("Failed to convert js value into rust type `String`")
        })
        .map(JsString::to_std_string_escaped)?)
}

mod kv {
    use boa_engine::{
        js_string, object::ObjectInitializer, property::Attribute, Context, JsError,
        JsNativeError, JsResult, JsString, JsValue, NativeFunction,
    };
    use boa_gc::{Finalize, Trace};
    use jstz_core::{api::Jstz, host::HostRuntime, with_jstz, GlobalApi};
    use std::{
        ops::{Deref, DerefMut},
        str::FromStr,
    };
    use tezos_smart_rollup::storage::path::OwnedPath;

    use super::string_from_arg;

    #[derive(Finalize, Trace)]
    pub struct Kv {
        prefix: String,
    }
    impl Kv {
        fn from_js<'a>(this: &'a JsValue) -> JsResult<impl DerefMut<Target = Self> + 'a> {
            Ok(this
                .as_object()
                .and_then(|obj| obj.downcast_mut::<Self>())
                .ok_or_else(|| {
                    JsError::from_native(
                        JsNativeError::typ().with_message(
                            "Failed to convert js value into rust type `Kv`",
                        ),
                    )
                })?)
        }
        fn key_path(&self, key: &String) -> OwnedPath {
            OwnedPath::try_from(format!("/{}/{}", self.prefix, key)).expect("")
        }
        fn set(&self, jstz: &mut Jstz, key: String, value: String) {
            jstz.transaction_mut()
                .insert(self.key_path(&key), value)
                .expect("")
        }
        fn get(
            &self,
            jstz: &mut Jstz,
            rt: &mut impl HostRuntime,
            key: String,
        ) -> Option<String> {
            jstz.transaction_mut()
                .get::<String>(rt, self.key_path(&key))
                .expect("")
                .cloned()
        }
        fn new(jstz: &Jstz) -> Self {
            Self {
                prefix: format!("test_storage/{}", jstz.self_address()),
            }
        }
    }
    pub struct Api;
    impl Api {
        const NAME: &str = "simpleKv";
        fn set(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let refr = Kv::from_js(this)?;
            let this = refr.deref();
            let key = string_from_arg(args, 0)?;
            let value = string_from_arg(args, 1)?;
            with_jstz!(context, [this.set](&mut jstz, key, value));
            Ok(JsValue::default())
        }
        fn get(
            this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let refr = Kv::from_js(this)?;
            let this = refr.deref();
            let key = string_from_arg(args, 0)?;
            let result = match with_jstz!(context, [this.get](&mut jstz, &mut hrt, key)) {
                None => JsValue::undefined(),
                Some(value) => JsString::from_str(&value).expect("infallable").into(),
            };
            Ok(result)
        }
    }
    impl GlobalApi for Api {
        fn init(context: &mut Context) {
            let native = with_jstz!(context, [Kv::new](&jstz,));
            let kv = ObjectInitializer::with_native(native, context)
                .function(NativeFunction::from_fn_ptr(Self::get), js_string!("get"), 1)
                .function(NativeFunction::from_fn_ptr(Self::set), js_string!("set"), 2)
                .build();

            context
                .register_global_property(js_string!(Self::NAME), kv, Attribute::all())
                .expect("kv api should only be registered once!")
        }
    }
}
mod console {
    use boa_engine::{
        js_string, object::ObjectInitializer, property::Attribute, Context, JsResult,
        JsValue, NativeFunction,
    };
    use jstz_core::{api::Jstz, host::HostRuntime, with_jstz, GlobalApi};
    use tezos_smart_rollup::prelude::debug_msg;

    use crate::refactored::string_from_arg;

    pub struct Console;
    impl Console {
        fn log(rt: &impl HostRuntime, message: String) {
            debug_msg!(rt, "[🪵] {message}\n")
        }
        fn log_with_address(jstz: &Jstz, rt: &impl HostRuntime, message: String) {
            let address = jstz.self_address();
            debug_msg!(rt, "[🪵] Contract at {address} says {message}\n")
        }
    }
    pub struct Api;
    impl Api {
        const NAME: &str = "simpleConsole";
        fn log(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let message = string_from_arg(args, 0)?;
            with_jstz!(context, [Console::log](&hrt, message));
            Ok(JsValue::default())
        }
        fn log_with_address(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let message = string_from_arg(args, 0)?;
            with_jstz!(context, [Console::log_with_address](&jstz, &hrt, message));
            Ok(JsValue::default())
        }
    }
    impl GlobalApi for Api {
        fn init(context: &mut Context) {
            let console = ObjectInitializer::with_native(&Console, context)
                .function(NativeFunction::from_fn_ptr(Self::log), js_string!("log"), 1)
                .function(
                    NativeFunction::from_fn_ptr(Self::log_with_address),
                    js_string!("logWithAddress"),
                    1,
                )
                .build();

            context
                .register_global_property(
                    js_string!(Self::NAME),
                    console,
                    Attribute::all(),
                )
                .expect("console api should only be registered once!")
        }
    }
}
pub struct Api;

impl jstz_core::GlobalApi for Api {
    fn init(context: &mut Context) {
        kv::Api::init(context);
        console::Api::init(context);
    }
}
