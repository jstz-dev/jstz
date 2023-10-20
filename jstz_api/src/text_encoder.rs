use boa_engine::{
    js_string,
    object::{
        builtins::{JsArrayBuffer, JsUint8Array},
        Object,
    },
    property::Attribute,
    Context, JsArgs, JsBigInt, JsNativeError, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    native::{register_global_class, ClassBuilder, JsNativeObject, NativeClass},
};

// https://encoding.spec.whatwg.org/#textencodercommon
//
// interface mixin TextEncoderCommon {
//   readonly attribute DOMString encoding;
// };
// dictionary TextEncoderEncodeIntoResult {
//   unsigned long long read;
//   unsigned long long written;
// };

// [Exposed=*]
// interface TextEncoder {
//   constructor();

//   [NewObject] Uint8Array encode(optional USVString input = "");
//   TextEncoderEncodeIntoResult encodeInto(USVString source, [AllowShared] Uint8Array destination);
// };
// TextEncoder includes TextEncoderCommon;

#[derive(Trace, Finalize)]
pub struct TextEncoder;

#[derive(Trace, Finalize)]
pub struct TextEncoderEncodeIntoResult {
    read: u128,
    written: u128,
}

impl TextEncoderEncodeIntoResult {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `TextEncoderEncodeIntoResult`")
                    .into()
            })
    }
}

impl Default for TextEncoderEncodeIntoResult {
    fn default() -> TextEncoderEncodeIntoResult {
        TextEncoderEncodeIntoResult {
            read: 0,
            written: 0,
        }
    }
}

impl NativeClass for TextEncoderEncodeIntoResult {
    type Instance = TextEncoderEncodeIntoResult;

    const NAME: &'static str = "TextEncoderEncodeIntoResult";

    fn constructor(
        _this: &JsNativeObject<TextEncoderEncodeIntoResult>,
        _args: &[JsValue],
        _: &mut Context<'_>,
    ) -> JsResult<TextEncoderEncodeIntoResult> {
        Ok(TextEncoderEncodeIntoResult::default())
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let read = accessor!(
            class.context(),
            TextEncoderEncodeIntoResult,
            "read",
            get:((x, _context) => Ok(JsBigInt::new(x.read).into()))
        );
        let written = accessor!(
            class.context(),
            TextEncoderEncodeIntoResult,
            "written",
            get:((x, _context) => Ok(JsBigInt::new(x.written).into()))
        );
        class
            .accessor(js_string!("read"), read, Attribute::all())
            .accessor(js_string!("written"), written, Attribute::all());

        Ok(())
    }
}

// Note: Using Box as a way to get a fixed size array, probably wrong
type Uint8Array = Box<[u8]>;

impl TextEncoder {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `TextEncoder`",
                    )
                    .into()
            })
    }

    fn encoding() -> &'static str {
        "utf-8"
    }

    fn encode(input: Option<String>) -> Uint8Array {
        let s = input.unwrap_or_else(|| "".to_string());
        s.into_bytes().into_boxed_slice()
    }

    fn encode_into(
        source: String,
        destination: &mut Uint8Array,
    ) -> TextEncoderEncodeIntoResult {
        let source = source.into_bytes().into_boxed_slice();
        let mut count = 0u128;
        for (i, dst_elt) in destination.iter_mut().enumerate() {
            if let Some(src_elt) = source.get(i) {
                count += 1;
                *dst_elt = *src_elt;
            }
        }
        TextEncoderEncodeIntoResult {
            read: count,
            written: count,
        }
    }
}
#[derive(Default, Clone, Trace, Finalize)]
pub struct TextEncoderApi;
impl TextEncoderApi {
    fn encode(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let arg = match args.get(0) {
            None => None,
            Some(arg) => {
                let arg = arg
                    .as_string()
                    .ok_or_else(|| JsNativeError::typ().with_message("expected string"))?
                    .to_std_string_escaped();
                Some(arg)
            }
        };
        let result = TextEncoder::encode(arg);
        let byte_block = result.to_vec();
        let array_buffer: JsArrayBuffer =
            JsArrayBuffer::from_byte_block(byte_block, context)?;
        let result_js = JsUint8Array::from_array_buffer(array_buffer, context)?;
        Ok(result_js.into())
    }

    fn encode_into(
        _: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let source = args
            .get_or_undefined(0)
            .try_js_into(context)
            .map_or(String::from(""), |x| x);
        let destination = args.get_or_undefined(1);
        // 2nd argument is a reference to a Uint8Array to be overwritten
        let init: JsUint8Array = destination.try_js_into(context)?;
        // initialize a boxed "destination buffer" with the transpiled content of the Js object
        let mut vec: Vec<u8> = Vec::with_capacity(init.length(context)?);
        for index in 0..init.length(context)? {
            vec.push(init.at(index as i64, context)?.to_uint8(context)?)
        }
        let destination_buffer: &mut Box<[u8]> = &mut Box::from(vec);
        // call the rust function
        let result: TextEncoderEncodeIntoResult =
            TextEncoder::encode_into(source, destination_buffer);
        // write back the destination JsValue
        for (i, value) in destination_buffer.iter().enumerate() {
            init.fill(*value, Some(i), Some(i + 1), context)?;
        }
        // create and return a TextEncoderEncodeIntoResult object
        let js_return: JsValue =
            JsNativeObject::new::<TextEncoderEncodeIntoResult>(result, context)
                .unwrap()
                .to_inner();
        Ok(js_return)
    }
}

impl NativeClass for TextEncoderApi {
    type Instance = TextEncoder;

    const NAME: &'static str = "TextEncoder";

    fn constructor(
        _this: &JsNativeObject<TextEncoder>,
        _args: &[JsValue],
        _: &mut Context<'_>,
    ) -> JsResult<TextEncoder> {
        Ok(TextEncoder)
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let encoding = accessor!(
            class.context(),
            TextEncoder,
            "encoding",
            get:((_x, _context) => Ok(JsString::from(TextEncoder::encoding()).into()))
        );
        class
            .accessor(js_string!("encoding"), encoding, Attribute::all())
            .method(
                js_string!("encode"),
                1,
                NativeFunction::from_fn_ptr(TextEncoderApi::encode),
            )
            .method(
                js_string!("encodeInto"),
                2,
                NativeFunction::from_fn_ptr(TextEncoderApi::encode_into),
            );

        Ok(())
    }
}

impl jstz_core::Api for TextEncoderApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<TextEncoderApi>(context)
            .expect("The `TextEncoder` class shouldn't exist yet");
        register_global_class::<TextEncoderEncodeIntoResult>(context)
            .expect("The `TextEncoderEncodeIntoResult` class shouldn't exist yet")
    }
}

/*
*/

/*
const encoder = new TextEncoder();
const bytes = new Uint8Array(2);
const result = encoder.encodeInto("\udc00", bytes);

result.read == 0;
result.written == 0;
*/
/*
var text = "H©ell©o Wor©ld!";
const encoder = new TextEncoder();
const encoded = encoder.encode(text);
const into = new Uint8Array(100);
const out = encoder.encodeInto(text, into);

out.read == text.length;
(encoded instanceof Uint8Array) == true;
*/

/*
const text = "Hello World!".repeat(1000);
const encoder = new TextEncoder();
const encoded = encoder.encode(text);

encoded instanceof Uint8Array == true;
encoded.length == text.length;
*/
