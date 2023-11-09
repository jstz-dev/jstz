use std::error::Error;

use base64::prelude::{Engine as _, BASE64_URL_SAFE as DEFAULT_ENGINE};
use boa_engine::{
    js_string, object::ObjectInitializer, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsResult, JsString, JsValue, NativeFunction,
};
use boa_gc::{Finalize, Trace};

#[derive(Trace, Finalize)]
struct TextEncoder;

impl TextEncoder {
    fn atob(data: &JsString) -> JsResult<JsString> {
        fn on_err(err: impl Error) -> JsError {
            JsNativeError::eval().with_message(err.to_string()).into()
        }
        let str = data.to_std_string_escaped();
        let encoded = DEFAULT_ENGINE.decode(str).map_err(on_err)?;
        let encoded_str = core::str::from_utf8(encoded.as_slice()).map_err(on_err)?;

        Ok(encoded_str.into())
    }
    fn btoa(data: &JsString) -> JsResult<JsString> {
        let str = data.to_std_string_escaped();
        let encoded = DEFAULT_ENGINE.encode(str);
        Ok(encoded.into())
    }
}

pub struct TextEncoderApi;
impl TextEncoderApi {
    const NAME: &str = "TextEncoder";
    fn atob(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let data = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("expected string"))?;
        let result = TextEncoder::atob(data)?;
        Ok(result.into())
    }
    fn btoa(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
        let data = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| JsNativeError::typ().with_message("expected string"))?;
        let result = TextEncoder::btoa(data)?;
        Ok(result.into())
    }
}

impl jstz_core::Api for TextEncoderApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        let encoding = ObjectInitializer::with_native(TextEncoder, context)
            .function(
                NativeFunction::from_fn_ptr(Self::atob),
                js_string!("atob"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::btoa),
                js_string!("btoa"),
                0,
            )
            .build();

        context
            .register_global_property(js_string!(Self::NAME), encoding, Attribute::all())
            .expect("TextEncoder api should only be registered once!")
    }
}
