use boa_engine::{
    js_string,
    object::{
        builtins::{JsArrayBuffer, JsUint8Array},
        ErasedObject,
    },
    property::Attribute,
    Context, JsArgs, JsBigInt, JsData, JsError, JsNativeError, JsResult, JsString,
    JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use encoding_rs::UTF_8;
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

#[derive(Trace, Finalize, JsData)]
pub struct TextEncoder;

#[derive(Trace, Finalize, JsData, Default, TryFromJs)]
pub struct TextEncoderEncodeIntoResult {
    read: u128,
    written: u128,
}

impl TextEncoderEncodeIntoResult {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
          .as_object()
          .and_then(|obj| obj.downcast_mut::<Self>())
          .ok_or_else(|| {
              JsNativeError::typ()
                  .with_message("Failed to convert js value into rust type `TextEncoderEncodeIntoResult`")
                  .into()
          })
    }
    fn read(context: &mut Context) -> Accessor {
        accessor!(
            context,
            TextEncoderEncodeIntoResult,
            "read",
            get:((x, _context) => Ok(JsBigInt::new(x.read).into()))
        )
    }
    fn written(context: &mut Context) -> Accessor {
        accessor!(
            context,
            TextEncoderEncodeIntoResult,
            "written",
            get:((x, _context) => Ok(JsBigInt::new(x.written).into()))
        )
    }
}

impl NativeClass for TextEncoderEncodeIntoResult {
    //  TODO: Wrt to dictionaries, given their copying semantics when being
    //  passed / returned in the WHATWG spec, in future they should be Rust
    //  structs that implement `TryFromJs` and `TryIntoJs`
    type Instance = TextEncoderEncodeIntoResult;

    const NAME: &'static str = "TextEncoderEncodeIntoResult";

    fn data_constructor(
        _target: &JsValue,
        _args: &[JsValue],
        _: &mut Context,
    ) -> JsResult<TextEncoderEncodeIntoResult> {
        Ok(TextEncoderEncodeIntoResult::default())
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        let read = Self::read(class.context());
        let written = Self::written(class.context());
        class
            .accessor(js_string!("read"), read, Attribute::all())
            .accessor(js_string!("written"), written, Attribute::all());

        Ok(())
    }
}

impl TextEncoder {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
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

    fn encoding(&self) -> String {
        String::from("utf-8")
    }

    fn encode(input: Option<&[u16]>) -> JsResult<Vec<u8>> {
        //  handle optional argument
        let input = input.unwrap_or(&[]);
        //  1. Convert input to an I/O queue of scalar values.

        //  2. Let output be the I/O queue of bytes << end-of-queue >>.
        //  NB: In Rust, we don't require end-of-queue item

        //  3. While true: ...
        //  NB: this is performed by the rust `encoding_rs` crate
        let mut encoder = UTF_8.new_encoder();

        // 3.1. Allocate a buffer with suffient space for the result
        let mut buffer: Vec<u8> = vec![
            0;
            encoder
                .max_buffer_length_from_utf16_if_no_unmappables(input.len())
                .ok_or_else(|| {
                    JsNativeError::eval().with_message("Input too large for buffer")
                })?
        ];

        // 3.2. Encode the input into the buffer using `encoding_rs`
        let (_result, _read, written, has_error) =
            encoder.encode_from_utf16(input, &mut buffer, true);

        // 3.3. Truncate the buffer to the number of bytes written
        buffer.truncate(written);

        if has_error {
            Err(JsNativeError::typ().with_message("Invalid input").into())
        } else {
            Ok(buffer)
        }
    }

    fn encode_into(
        source: &[u16],
        destination: &mut [u8],
    ) -> JsResult<TextEncoderEncodeIntoResult> {
        //  1. Let read be 0
        //  2. Let written be 0.
        //  NB: read and written are both directly computed by `encode_from_utf16`

        //  3. Let encoder be an instance of the UTF-8 encoder
        //  NB: Noop, we only support UTF-8 encoder
        //  4. Let unused be the I/O queue of scalar values << end-of-queue >>.
        //  NB: The handler algorithm invoked below requires this argument, but it is not used by the UTF-8 encoder.
        //  Therefore Noop
        //  5. Convert src to an I/O queue of scalar values.
        //  6. While true: ...
        //  NB: we don't use an I/O queue here and we use `encode_from_utf16``
        //      the u16 slice will be considered malformed (`has_error`) if
        //      any unpaired surrogates

        let mut encoder = UTF_8.new_encoder();

        let (_result, read, written, has_error) =
            encoder.encode_from_utf16(source, destination, true);

        if has_error {
            Err(JsNativeError::typ().with_message("Invalid input").into())
        } else {
            Ok(TextEncoderEncodeIntoResult {
                read: read as u128,
                written: written as u128,
            })
        }
    }
}

#[derive(Default, Clone, Trace, Finalize, JsData)]
pub struct TextEncoderClass;
impl TextEncoderClass {
    fn encoding(context: &mut Context) -> Accessor {
        accessor!(
            context,
            TextEncoder,
            "encoding",
            get:((this, _context) => Ok(JsString::from(this.encoding()).into()))
        )
    }

    fn encode(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let input = args.get_or_undefined(0).as_string().map(JsString::to_vec);

        let result = TextEncoder::encode(input.as_deref())?;

        let uint8_array = JsUint8Array::from_array_buffer(
            JsArrayBuffer::from_byte_block(result, context)?,
            context,
        )?;

        Ok(uint8_array.into())
    }

    fn encode_into(
        _: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .map(JsString::to_vec)
            .ok_or::<JsError>(
                JsNativeError::typ()
                    .with_message("Failed to interpret argument as a string")
                    .into(),
            )?;

        let dst: JsUint8Array = args.get_or_undefined(1).try_js_into(context)?;
        let dst_buffer: JsArrayBuffer = dst.buffer(context)?.try_js_into(context)?;
        let mut dst_slice = dst_buffer.data_mut();

        let result: TextEncoderEncodeIntoResult =
            TextEncoder::encode_into(&src, dst_slice.as_deref_mut().unwrap_or_default())?;

        // create and return a TextEncoderEncodeIntoResult object
        let js_return: JsValue =
            JsNativeObject::new::<TextEncoderEncodeIntoResult>(result, context)
                .expect("Expect `TextEncoderEncodeIntoResult` to be constructible")
                .to_inner();
        Ok(js_return)
    }
}

impl NativeClass for TextEncoderClass {
    type Instance = TextEncoder;

    const NAME: &'static str = "TextEncoder";

    fn data_constructor(
        _target: &JsValue,
        _args: &[JsValue],
        _: &mut Context,
    ) -> JsResult<TextEncoder> {
        Ok(TextEncoder)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        let encoding = Self::encoding(class.context());
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
    fn init(self, context: &mut Context) {
        register_global_class::<TextEncoderClass>(context)
            .expect("The `TextEncoder` class shouldn't exist yet");
        register_global_class::<TextEncoderEncodeIntoResult>(context)
            .expect("The `TextEncoderEncodeIntoResult` class shouldn't exist yet")
    }
}
