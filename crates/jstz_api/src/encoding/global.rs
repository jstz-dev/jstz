use std::error::Error;

use base64::prelude::{Engine as _, BASE64_URL_SAFE as DEFAULT_ENGINE};
use boa_engine::{
    js_string, Context, JsArgs, JsError, JsNativeError, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{Finalize, Trace};

#[derive(Trace, Finalize)]
struct Global;

impl Global {
    fn atob(data: &JsString) -> JsResult<JsString> {
        fn on_err(err: impl Error) -> JsError {
            JsNativeError::eval().with_message(err.to_string()).into()
        }
        let str = data.to_std_string_escaped();
        let encoded = DEFAULT_ENGINE.decode(&str).map_err(on_err)?;
        let encoded_str = core::str::from_utf8(encoded.as_slice()).map_err(on_err)?;

        Ok(encoded_str.into())
    }
    fn btoa(data: &JsString) -> JsResult<JsString> {
        let str = data.to_std_string_escaped();
        let encoded = DEFAULT_ENGINE.encode(&str);
        Ok(encoded.into())
    }
}

pub struct GlobalApi;
impl GlobalApi {
    fn atob(_: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
        let data: &JsString = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("expected string"))?;
        // TODO: https://github.com/trilitech/jstz/pull/197#discussion_r1413836707
        // let data: JsString = args.get_or_undefined(0).try_js_into(context);
        let result = Global::atob(&data)?;
        Ok(result.into())
    }
    fn btoa(_: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
        let data: &JsString = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("expected string"))?;
        // TODO: https://github.com/trilitech/jstz/pull/197#discussion_r1413836707
        let result = Global::btoa(&data)?;
        Ok(result.into())
    }
}

impl jstz_core::Api for GlobalApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        context
            .register_global_builtin_callable(
                js_string!("atob"),
                1,
                NativeFunction::from_fn_ptr(GlobalApi::atob),
            )
            .expect("atob should only be registered once");
        context
            .register_global_builtin_callable(
                js_string!("btoa"),
                1,
                NativeFunction::from_fn_ptr(GlobalApi::btoa),
            )
            .expect("btoa should only be registered once");
    }
}
