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
use encoding::all::UTF_8;
use encoding::{EncoderTrap, Encoding};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::TryFromJs,
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

#[derive(Trace, Finalize, Default, TryFromJs)]
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
    fn read(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextEncoderEncodeIntoResult,
            "read",
            get:((x, _context) => Ok(JsBigInt::new(x.read).into()))
        )
    }
    fn written(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextEncoderEncodeIntoResult,
            "written",
            get:((x, _context) => Ok(JsBigInt::new(x.written).into()))
        )
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
        let read = Self::read(class.context());
        let written = Self::written(class.context());
        class
            .accessor(js_string!("read"), read, Attribute::all())
            .accessor(js_string!("written"), written, Attribute::all());

        Ok(())
    }
}

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

    fn encoding(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextEncoder,
            "encoding",
            get:((_x, _context) => Ok(JsString::from("utf-8").into()))
        )
    }

    fn encode(input: Option<&[u16]>) -> Vec<u8> {
        //  handle optional argument
        let input = input.unwrap_or_else(|| &[]);
        //  1. Convert input to an I/O queue of scalar values.
        //  NB: here we just convert a u16 slice to a string slice using rust `String::from_utf16_lossy`
        //      it seems `encode` reads unpaired surrogates (i.e. U+DC00) as replacement character U+FFFD
        let input = String::from_utf16_lossy(input);
        let input: &str = input.as_str();
        //  2. Let output be the I/O queue of bytes << end-of-queue >>.
        //  NB: In Rust, we don't require end-of-queue item

        //  3. While true: ...
        //  NB: this is performed by the rust `encoding` crate. The "fatal" ErrorMode corresponds to `EncoderTrap::Strict`
        UTF_8.encode(input, EncoderTrap::Strict).unwrap()
    }

    fn custom_decode_utf16(input: &[u16]) -> String {
        let input = input.into_iter().map(|&codepoint| codepoint);
        let mut output: Vec<char> = Vec::new();
        let mut utf16_decoder = char::decode_utf16(input);
        while let Some(result) = utf16_decoder.next() {
            match result {
                Ok(s) => output.push(s),
                Err(_) => break,
            }
        }
        output.into_iter().collect()
    }

    fn encode_into(
        source: &[u16],
        destination: &mut Vec<u8>,
    ) -> TextEncoderEncodeIntoResult {
        //  1. Let read be 0
        //  2. Let written be 0.
        //  NB: read and written both correspond to the same value as UTF encoder can't fail
        //      they are later set to `source` length
        let read = source.len() as u128;
        let written = read;

        //  3. Let encoder be an instance of the UTF-8 encoder
        //  NB: Noop, we only support UTF-8 encoder
        //  4. Let unused be the I/O queue of scalar values << end-of-queue >>.
        //  NB: The handler algorithm invoked below requires this argument, but it is not used by the UTF-8 encoder.
        //  Therefore Noop
        //  5. Convert src to an I/O queue of scalar values.
        //  NB: here we just convert a u16 slice to a string slice using our custom decoder
        //      it will stop after the first unpaired surrogate
        let source = Self::custom_decode_utf16(source);
        let input = source.as_str();
        //  6. While true: ...
        //  NB: this is performed by the rust `encoding` crate.
        let () = UTF_8
            .encode_to(input, EncoderTrap::Call(|_, _, _| false), destination)
            .unwrap();
        TextEncoderEncodeIntoResult { read, written }
    }
}
#[derive(Default, Clone, Trace, Finalize)]
pub struct TextEncoderClass;
impl TextEncoderClass {
    fn encode(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let arg = args.get_or_undefined(0).as_string().map(|x| x.as_slice());
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
            .as_string()
            .map(|x| x.as_slice())
            .unwrap();
        let destination = args.get_or_undefined(1);
        // 2nd argument is a reference to a Uint8Array to be overwritten
        let init: JsUint8Array = destination.try_js_into(context)?;
        // initialize a boxed "destination buffer" with the transpiled content of the Js object
        let destination: &mut Vec<u8> = &mut Vec::new();
        // call the rust function
        let result: TextEncoderEncodeIntoResult =
            TextEncoder::encode_into(source, destination);
        // write back the destination JsValue
        for (i, value) in destination.iter().enumerate() {
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

impl NativeClass for TextEncoderClass {
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
        let encoding = TextEncoder::encoding(class.context());
        class
            .accessor(js_string!("encoding"), encoding, Attribute::all())
            .method(
                js_string!("encode"),
                1,
                NativeFunction::from_fn_ptr(TextEncoderClass::encode),
            )
            .method(
                js_string!("encodeInto"),
                2,
                NativeFunction::from_fn_ptr(TextEncoderClass::encode_into),
            );

        Ok(())
    }
}

pub struct TextEncoderApi;
impl jstz_core::Api for TextEncoderApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<TextEncoderClass>(context)
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

/*
const encoder = new TextEncoder();
encoder.encode("\udc00");
*/
