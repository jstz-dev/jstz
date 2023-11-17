use std::io::Write;

use boa_engine::{
    js_string,
    object::{
        builtins::{JsArrayBuffer, JsInt8Array, JsUint8Array},
        Object,
    },
    property::Attribute,
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use encoding_rs::{CoderResult, Decoder, Encoding};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::{IntoJs, TryFromJs},
};
// https://encoding.spec.whatwg.org/#interface-textdecoder
//
// dictionary TextDecoderOptions {
//   boolean fatal = false;
//   boolean ignoreBOM = false;
// };

// dictionary TextDecodeOptions {
//   boolean stream = false;
// };

// [Exposed=*]
// interface TextDecoder {
//   constructor(optional DOMString label = "utf-8", optional TextDecoderOptions Options = {});

//   USVString decode(optional AllowSharedBufferSource input, optional TextDecodeOptions Options = {});
// };
// TextDecoder includes TextDecoderCommon;

// https://encoding.spec.whatwg.org/#interface-mixin-textdecodercommon
// interface mixin TextDecoderCommon {
//   readonly attribute DOMString encoding;
//   readonly attribute boolean fatal;
//   readonly attribute boolean ignoreBOM;
// };

#[derive(Trace, Finalize)]
pub struct TextDecoder {
    encoding: &'static Encoding,
    #[unsafe_ignore_trace]
    decoder: Decoder,
    io_queue: Vec<u8>,
    ignore_bom: bool,
    error_mode: String,
    do_not_flush: bool,
}
#[derive(Trace, Finalize, Default)]
pub struct TextDecoderOptions {
    fatal: bool,
    ignore_bom: bool,
}

impl TryFromJs for TextDecoderOptions {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected `JsObject`"))
        })?;
        let def = TextDecoderOptions::default();
        let fatal = if obj.has_property(js_string!("fatal"), context)? {
            obj.get(js_string!("fatal"), context)?
                .try_js_into(context)?
        } else {
            def.fatal
        };
        let ignore_bom = if obj.has_property(js_string!("ignoreBOM"), context)? {
            obj.get(js_string!("ignoreBOM"), context)?
                .try_js_into(context)?
        } else {
            def.ignore_bom
        };
        Ok(Self { fatal, ignore_bom })
    }
}

#[derive(Trace, Finalize, Default)]
pub struct TextDecodeOptions {
    stream: bool,
}

impl TryFromJs for TextDecodeOptions {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected `JsObject`"))
        })?;
        let def = TextDecodeOptions::default();
        let stream = if obj.has_property(js_string!("stream"), context)? {
            obj.get(js_string!("stream"), context)?
                .try_js_into(context)?
        } else {
            def.stream
        };
        Ok(Self { stream })
    }
}

impl TextDecoder {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `TextDecoder`",
                    )
                    .into()
            })
    }

    //  https://encoding.spec.whatwg.org/#dom-textdecoder
    fn constructor(
        label: Option<String>,
        options: Option<TextDecoderOptions>,
    ) -> Result<TextDecoder, ()> {
        //  Handle optional parameters
        let label = label.unwrap_or("utf-8".to_string());
        let options = options.unwrap_or(TextDecoderOptions::default());
        //  The new TextDecoder(label, options) constructor steps are:
        //  1. Let encoding be the result of getting an encoding from label.
        //  2. If encoding is failure or replacement, then throw a RangeError.
        let encoding =
            encoding_rs::Encoding::for_label_no_replacement(label.as_bytes()).ok_or(())?;
        Ok(TextDecoder {
            //  3. Set this's encoding to encoding.
            encoding: encoding,
            //  4. If options["fatal"] is true, then set this's error mode to "fatal".
            error_mode: if options.fatal {
                "fatal".to_string()
            } else {
                "replacement".to_string()
            },
            decoder: if options.ignore_bom {
                encoding.new_decoder_without_bom_handling()
            } else {
                encoding.new_decoder()
            },
            io_queue: Vec::new(),
            //  5. Set this's ignore BOM to options["ignoreBOM"].
            ignore_bom: options.ignore_bom,
            //  A TextDecoder object has an associated do not flush, which is a boolean, initially false.
            do_not_flush: false,
        })
    }
    fn encoding(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "encoding",
            get:((x, context) => Ok(x.encoding.name().to_string().to_lowercase().into_js(context)))
        )
    }
    fn fatal(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "fatal",
            get:((x, _context) => Ok((x.error_mode == "fatal").into()))
        )
    }
    fn ignore_bom(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "ignoreBOM",
            get:((x, _context) => Ok(x.ignore_bom.into()))
        )
    }

    //  https://encoding.spec.whatwg.org/#concept-td-serialize
    fn serialize(&mut self, read: usize) -> () {
        //  1. Let output be the empty string.
        //  2. While true:
        //    1. Let item be the result of reading from ioQueue.
        //    2. If item is end-of-queue, then return output.
        //    3. If decoder’s encoding is UTF-8 or UTF-16BE/LE, and decoder’s ignore BOM and BOM seen are false, then:
        //      1. Set decoder’s BOM seen to true.
        //      2. If item is U+FEFF, then continue.
        //  4. Append item to output.

        //  NB: we call this function in case of a chunked input with `written`:
        //      https://docs.rs/encoding_rs/latest/encoding_rs/struct.Decoder.html
        //      The number of bytes “written” is what’s logically written. Garbage may
        //      be written in the output buffer beyond the point logically written to.
        //  NB: we handle BOM at the level of the decoding function from `::encoding_rs` crate
        self.io_queue.drain(0..read);
    }

    //  https://encoding.spec.whatwg.org/#dom-textdecoder-decode
    fn decode(
        // input: Option<allowsharedbufersource>,
        &mut self,
        input: Option<Vec<u8>>,
        options: Option<TextDecodeOptions>,
    ) -> JsResult<String> {
        //  Handle optional parameters
        let input = input.unwrap_or(vec![]);
        let options = options.unwrap_or(TextDecodeOptions::default());
        //  1. If this's do not flush is false,
        if !self.do_not_flush {
            // println!("Flushing..");
            //  then set this's decoder to a new instance of this's encoding's decoder,
            self.decoder = if self.ignore_bom {
                self.encoding.new_decoder_without_bom_handling()
            } else {
                self.encoding.new_decoder()
            };
            //  this's I/O queue to the I/O queue of bytes <<end-of-queue>>,
            self.io_queue.clear();
            //  and this's BOM seen to false.
            //  NB: we do not need this flag (intern to `encoding_rs` crate decode functions)
        };
        //  2. Set this's do not flush to options["stream"].
        self.do_not_flush = options.stream;
        //  3. If input is given, then push a copy of input to this's I/O queue.
        //  FIXME: map this error into a JsError ? which one ?
        // println!("io_queue {:?}", self.io_queue);
        let _: Result<usize, std::io::Error> = self.io_queue.write(input.as_slice());
        // println!("io_queue {:?}", self.io_queue);

        //  4. Let output be the I/O queue of scalar values <<end-of-queue>>.
        let mut output_buffer = [0u8; 2048];
        //  5. While true:
        //    1. Let item be the result of reading from this's I/O queue.
        //    2. If item is end-of-queue and this's do not flush is true,
        //       then return the result of running serialize I/O queue with this and output.
        //    3. Otherwise:
        //      1. Let result be the result of processing an item with item, this's decoder,
        //         this's I/O queue, output, and this's error mode.
        //      2. If result is finished, then return the result of running serialize I/O queue with this and output.
        //      3. Otherwise, if result is error, throw a TypeError.

        //  NB: the above is implemented using the encoding_rs crate:
        //    - in the case of a chunked input, we use the decoder's decode_to* method
        //      and we apply `self.serialize` to maintain a state in `self.io_queue`
        //    - in the case of a plain input, we use `self.encoding.decode` on `self.io_queue`
        // FIXME: ignore_bom should be used somewhere
        let (result, had_errors) = if self.do_not_flush {
            // chunked input
            let (result, read, written, had_errors) =
                self.decoder
                    .decode_to_utf8(&self.io_queue, &mut output_buffer, false);
            match result {
                // FIXME: result can indicate buffer size problem ? should we worry about that ?
                CoderResult::InputEmpty => (),
                CoderResult::OutputFull => (),
            };
            let output = output_buffer[..written].to_vec();
            // println!("result {:?}", result);
            // println!("read {:?}", read);
            // println!("written {:?}", written);
            // println!("had_errors {:?}", had_errors);
            // println!("output {:?}", output);
            self.serialize(read);
            (String::from_utf8(output).unwrap(), had_errors)
        } else {
            // plain input
            let (_result, _read, written, had_errors) =
                self.decoder
                    .decode_to_utf8(&self.io_queue, &mut output_buffer, true);
            let output = output_buffer[..written].to_vec();
            (String::from_utf8(output).unwrap(), had_errors)
        };
        if had_errors && self.error_mode == "fatal" {
            return Err(JsError::from_native(
                JsNativeError::typ().with_message("TypeError"),
            ));
        };
        Ok(result)
    }
}

#[derive(Default, Clone, Trace, Finalize)]
pub struct TextDecoderClass;
impl TextDecoderClass {
    fn decode(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut text_decoder = TextDecoder::try_from_js(this)?;
        let input = args.get_or_undefined(0);
        let input = allow_shared_buffer_source(input, context)?;
        let options = args.get_or_undefined(1).try_js_into(context)?;
        text_decoder
            .decode(input, options)
            .map(|x| JsString::from(x).into())
    }
}

impl NativeClass for TextDecoderClass {
    type Instance = TextDecoder;

    const NAME: &'static str = "TextDecoder";

    fn constructor(
        _this: &JsNativeObject<TextDecoder>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<TextDecoder> {
        let label: Option<String> = args.get_or_undefined(0).try_js_into(context)?;
        let options: Option<TextDecoderOptions> =
            args.get_or_undefined(1).try_js_into(context)?;
        TextDecoder::constructor(label, options).map_err(|()| {
            JsNativeError::range()
                .with_message("Failed to construct 'TextDecoder'")
                .into()
        })
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let encoding = TextDecoder::encoding(class.context());
        let fatal = TextDecoder::fatal(class.context());
        let ignore_bom = TextDecoder::ignore_bom(class.context());
        class
            .accessor(js_string!("encoding"), encoding, Attribute::all())
            .accessor(js_string!("fatal"), fatal, Attribute::all())
            .accessor(js_string!("ignoreBOM"), ignore_bom, Attribute::all())
            .method(
                js_string!("decode"),
                1,
                NativeFunction::from_fn_ptr(TextDecoderClass::decode),
            );

        Ok(())
    }
}

pub struct TextDecoderApi;
impl jstz_core::Api for TextDecoderApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<TextDecoderClass>(context)
            .expect("The `TextDecoder` class shouldn't exist yet");
    }
}

// TODO: put that in some shared space between APIs ?
fn allow_shared_buffer_source<'a>(
    js_value: &JsValue,
    context: &mut Context<'_>,
) -> JsResult<Option<Vec<u8>>> {
    if js_value.is_undefined() {
        return Ok(None);
    }
    let js_object: JsObject = js_value.to_object(context)?;
    if js_object.is_array_buffer() {
        let arr: JsArrayBuffer = js_value.try_js_into(context)?;
        arr.take().map(Some)
    } else if js_object.is_typed_array() {
        if js_object.is_typed_uint8_array() {
            let arr: JsUint8Array = js_value.try_js_into(context)?;
            let mut vec = vec![];
            let length = arr.length(context)?;
            for i in 0..length {
                let x = arr.get(i, context)?.to_uint8(context)?;
                vec.push(x)
            }
            Ok(Some(vec))
        } else if js_object.is_typed_int8_array() {
            let arr: JsInt8Array = js_value.try_js_into(context)?;
            let mut vec = vec![];
            let length = arr.length(context)?;
            for i in 0..length {
                let x = arr.get(i, context)?.to_uint8(context)?;
                vec.push(x)
            }
            Ok(Some(vec))
        } else {
            todo!()
        }
    } else if js_object.is_data_view() {
        // FIXME
        Ok(None)
    } else {
        // FIXME
        Ok(None)
    }
}

/*

  const decoder = new TextDecoder();
  decoder.decode(new Uint8Array([0xF0, 0x9F]), { stream: true });
  decoder.decode(new Uint8Array([0x41]), { stream: true });
  decoder.decode() == "";

  1 | const decoder = new TextDecoder();
  2 | decoder.decode(new Uint8Array([0xF0, 0x9F]), { stream: true }) == "";
  3 | decoder.decode(new Uint8Array([0x41]), { stream: true }) == "\uFFFDA";
  4 | decoder.decode() == "";


  when processing line 3,

  ioqueue = 0xF09F

  11110000 10011111 01000001

  [11110] 000   -> expecting 4-bytes sequence
  [10] 011111   -> 1st byte of the sequence
  [0] 1000001   -> malformed (do not start with 10) pushing U+FFFD (replacement char.) and 1000001

===========================

  const decoder = new TextDecoder();
  decoder.decode(new Uint8Array([0xF0, 0x41, 0xF0]), { stream: true }) == "\uFFFDA";
  decoder.decode() == "\uFFFD";



*/
/*

const createBuffer = (() => {
  // See https://github.com/whatwg/html/issues/5380 for why not `new SharedArrayBuffer()`
  let sabConstructor;
  try {
    sabConstructor = new WebAssembly.Memory({ shared:true, initial:0, maximum:0 }).buffer.constructor;
  } catch(e) {
    sabConstructor = null;
  }
  return (type, length, opts) => {
    if (type === "ArrayBuffer") {
      return new ArrayBuffer(length, opts);
    } else if (type === "SharedArrayBuffer") {
      if (sabConstructor && sabConstructor.name !== "SharedArrayBuffer") {
        throw new Error("WebAssembly.Memory does not support shared:true");
      }
      return new sabConstructor(length, opts);
    } else {
      throw new Error("type has to be ArrayBuffer or SharedArrayBuffer");
    }
  }
})();
const arrayBufferOrSharedArrayBuffer = "ArrayBuffer"
const buf = createBuffer(arrayBufferOrSharedArrayBuffer, 2);
const view = new Uint8Array(buf);
const buf2 = createBuffer(arrayBufferOrSharedArrayBuffer, 2);
const view2 = new Uint8Array(buf2);
const decoder = new TextDecoder("utf-8");
view[0] = 0xEF;
view[1] = 0xBB;
view2[0] = 0xBF;
view2[1] = 0x40;

decoder.decode(buf, {stream:true})

view[0] = 0x01;
view[1] = 0x02;
decoder.decode(buf2)

*/
