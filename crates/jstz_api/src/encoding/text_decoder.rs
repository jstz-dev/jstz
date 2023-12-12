use std::io::Write;

use boa_engine::{
    js_string, object::Object, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use encoding_rs::{Decoder, DecoderResult, Encoding};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::{IntoJs, TryFromJs},
};

use crate::idl::{ArrayBufferLike, JsBufferSource};
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
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
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
    fn new(
        label: Option<String>,
        options: Option<TextDecoderOptions>,
    ) -> Result<TextDecoder, ()> {
        //  Handle optional parameters
        let label = label.unwrap_or("utf-8".to_string());
        let options = options.unwrap_or_default();

        //  The new TextDecoder(label, options) constructor steps are:
        //  1. Let encoding be the result of getting an encoding from label.
        //  2. If encoding is failure or replacement, then throw a RangeError.
        let encoding = Encoding::for_label_no_replacement(label.as_bytes()).ok_or(())?;

        Ok(TextDecoder {
            //  3. Set this's encoding to encoding.
            encoding,
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

    fn encoding(&self) -> String {
        self.encoding.name().to_lowercase()
    }

    fn fatal(&self) -> bool {
        self.error_mode == "fatal"
    }

    fn ignore_bom(&self) -> bool {
        self.ignore_bom
    }

    //  https://encoding.spec.whatwg.org/#concept-td-serialize
    fn serialize(&mut self, read: usize) {
        //  1. Let output be the empty string.
        //  2. While true:
        //    1. Let item be the result of reading from ioQueue.
        //    2. If item is end-of-queue, then return output.
        //    3. If decoder’s encoding is UTF-8 or UTF-16BE/LE, and decoder’s ignore BOM and BOM seen are false, then:
        //      1. Set decoder’s BOM seen to true.
        //      2. If item is U+FEFF, then continue.
        //  4. Append item to output.

        //  NB: we call this function in case of a chunked input with `read`:
        //      https://docs.rs/encoding_rs/latest/encoding_rs/struct.Decoder.html
        //  NB: BOM handling is done at the level of the decoding function from `::encoding_rs` crate
        self.io_queue.drain(0..read);
    }

    //  https://encoding.spec.whatwg.org/#dom-textdecoder-decode
    fn decode(
        &mut self,
        input: Option<&[u8]>,
        options: Option<TextDecodeOptions>,
    ) -> JsResult<Vec<u16>> {
        //  Handle optional parameters
        let input = input.unwrap_or_default();
        let options = options.unwrap_or_default();

        //  1. If this's do not flush is false,
        if !self.do_not_flush {
            //  then set this's decoder to a new instance of this's encoding's decoder,
            self.decoder = if self.ignore_bom {
                self.encoding.new_decoder_without_bom_handling()
            } else {
                self.encoding.new_decoder()
            };
            //  this's I/O queue to the I/O queue of bytes <<end-of-queue>>,
            self.io_queue.clear();
            //  and this's BOM seen to false.
            //  NB: we do not need this flag. The `Encoding` object from
            //      `encoding_rs` crate already embeds this
        };

        //  2. Set this's do not flush to options["stream"].
        self.do_not_flush = options.stream;

        //  3. If input is given, then push a copy of input to this's I/O queue.
        self.io_queue.write(input).map_err(|_| {
            JsNativeError::eval()
                .with_message("IO error when writing to IO queue in TextDecoder")
        })?;

        //  4. Let output be the I/O queue of scalar values <<end-of-queue>>.
        let mut output: Vec<u16> = Vec::with_capacity(
            self.decoder
                .max_utf16_buffer_length(input.len())
                .expect("If usize overflows, then we cannot alloc this"),
        );
        output.resize(output.capacity(), 0);

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
        //    - we use the decoder's decode_to* method and we apply `self.serialize`
        //      to maintain a state in `self.io_queue` in case of a chunked input

        let (read, written) = if self.fatal() {
            let (result, read, written) =
                self.decoder.decode_to_utf16_without_replacement(
                    &self.io_queue,
                    &mut output,
                    !self.do_not_flush,
                );

            if matches!(result, DecoderResult::Malformed(_, _)) {
                return Err(JsError::from_native(
                    JsNativeError::typ().with_message("TypeError"),
                ));
            };

            (read, written)
        } else {
            let (_result, read, written, _had_errors) = self.decoder.decode_to_utf16(
                &self.io_queue,
                &mut output,
                !self.do_not_flush,
            );

            (read, written)
        };

        self.serialize(read);
        output.truncate(written);

        Ok(output)
    }
}

#[derive(Default, Clone, Trace, Finalize)]
pub struct TextDecoderClass;
impl TextDecoderClass {
    fn encoding(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "encoding",
            get:((this, context) => Ok(this.encoding().into_js(context)))
        )
    }
    fn fatal(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "fatal",
            get:((this, _context) => Ok(this.fatal().into()))
        )
    }
    fn ignore_bom(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            TextDecoder,
            "ignoreBOM",
            get:((this, _context) => Ok(this.ignore_bom().into()))
        )
    }

    fn decode(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut text_decoder = TextDecoder::try_from_js(this)?;
        let input: Option<JsBufferSource> =
            args.get_or_undefined(0).try_js_into(context)?;
        let options = args.get_or_undefined(1).try_js_into(context)?;

        // TODO: BORROW CHECKER ISSUE
        // We cannot borrow the slice directly from `input` because we need to create
        // 2 temporaries: a `JsArrayBufferData` and a `GcRefMut`.
        let result = match input {
            Some(input) => {
                let array_buffer_data = input.to_array_buffer_data(context)?;
                let array_buffer_slice = array_buffer_data.as_slice_mut();
                text_decoder.decode(array_buffer_slice.as_deref(), options)
            }
            None => text_decoder.decode(None, options),
        }?;

        Ok(js_string!(result).into())
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
        TextDecoder::new(label, options).map_err(|()| {
            JsNativeError::range()
                .with_message("Failed to construct 'TextDecoder'")
                .into()
        })
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let encoding = Self::encoding(class.context());
        let fatal = Self::fatal(class.context());
        let ignore_bom = Self::ignore_bom(class.context());
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
