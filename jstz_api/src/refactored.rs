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
    use jstz_core::{
        api::TezosObject, host::HostRuntime, runtime::with_global_host, tezos_object,
        GlobalApi,
    };
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
        fn set(&self, tezos: &mut TezosObject, key: String, value: String) {
            tezos
                .transaction_mut()
                .insert(self.key_path(&key), value)
                .expect("")
        }
        fn get(
            &self,
            tezos: &mut TezosObject,
            rt: &mut impl HostRuntime,
            key: String,
        ) -> Option<String> {
            tezos
                .transaction_mut()
                .get::<String>(rt, self.key_path(&key))
                .expect("")
                .cloned()
        }
        fn new(tezos: &TezosObject) -> Self {
            Self {
                prefix: format!("test_storage/{}", tezos.self_address()),
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
            tezos_object!(mut tezos, context);
            this.set(&mut tezos, key, value);
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
            tezos_object!(mut tezos, context);
            let result = match with_global_host(|hrt| this.get(&mut tezos, hrt, key)) {
                None => JsValue::undefined(),
                Some(value) => JsString::from_str(&value).expect("infallable").into(),
            };
            Ok(result)
        }
    }
    impl GlobalApi for Api {
        fn init(context: &mut Context) {
            tezos_object!(tezos, context);
            let native = Kv::new(&tezos);
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
    use jstz_core::{
        api::TezosObject, host::HostRuntime, runtime::with_global_host, tezos_object,
        GlobalApi,
    };
    use tezos_smart_rollup::prelude::debug_msg;

    use crate::refactored::string_from_arg;

    pub struct Console;
    impl Console {
        fn log(rt: &impl HostRuntime, message: String) {
            debug_msg!(rt, "[🪵] {message}\n")
        }
        fn log_with_address(tezos: &TezosObject, rt: &impl HostRuntime, message: String) {
            let address = tezos.self_address();
            debug_msg!(rt, "[🪵] Contract at {address} says {message}\n")
        }
    }
    pub struct Api;
    impl Api {
        const NAME: &str = "simpleConsole";
        fn log(
            _this: &JsValue,
            args: &[JsValue],
            _context: &mut Context,
        ) -> JsResult<JsValue> {
            let message = string_from_arg(args, 0)?;
            with_global_host(|hrt| Console::log(hrt, message));
            Ok(JsValue::default())
        }
        fn log_with_address(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let message = string_from_arg(args, 0)?;
            tezos_object!(tezos, context);
            with_global_host(|hrt| Console::log_with_address(&tezos, hrt, message));
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
