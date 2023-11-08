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
use encoding::{label, EncoderTrap, Encoding};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::TryFromJs,
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
//   constructor(Optional DOMString label = "utf-8", Optional TextDecoderOptions Options = {});

//   USVString decode(Optional AllowSharedBufferSource input, Optional TextDecodeOptions Options = {});
// };
// TextDecoder includes TextDecoderCommon;

#[derive(Trace, Finalize)]
pub struct TextDecoder {
    // https://encoding.spec.whatwg.org/#interface-mixin-textdecodercommon
    // interface mixin TextDecoderCommon {
    //   readonly attribute DOMString encoding;
    //   readonly attribute boolean fatal;
    //   readonly attribute boolean ignoreBOM;
    // };
    encoding: String,
    fatal: bool,
    ignoreBOM: bool,
}
#[derive(Trace, Finalize, Default, TryFromJs)]
pub struct TextDecoderOptions {
    fatal: bool,
    ignoreBOM: bool,
}

#[derive(Trace, Finalize, Default, TryFromJs)]
pub struct TextDecodeOptions {
    stream: bool,
}

// https://webidl.spec.whatwg.org/#idl-USVString
// The USVString type corresponds to scalar value strings [...]
// https://infra.spec.whatwg.org/#scalar-value-string
// A scalar value string is a string whose code points are all scalar values [...]
// A scalar value is a code point that is not a surrogate [...]
// To convert a string into a scalar value string, replace any surrogates with U+FFFD [...]

impl TextDecoder {
    fn constructor(
        label: Option<String>,
        options: Option<TextDecoderOptions>,
    ) -> Result<TextDecoder, ()> {
        //  Handle optional parameters
        let label = label.unwrap_or("utf-8".to_string());
        let options = options.unwrap_or(TextDecoderOptions::default());
        // The new TextDecoder(label, options) constructor steps are:
        //  1. Let encoding be the result of getting an encoding from label.
        let encoding: Option<&(dyn Encoding + Send + Sync)> =
            label::encoding_from_whatwg_label(&label);
        //  2. If encoding is failure or replacement, then throw a RangeError.
        let encoding: &(dyn Encoding + Send + Sync) = match encoding {
            Some(x) if x.whatwg_name().is_some_and(|x| x == "replacement") => {
                Err(todo!())
            }
            Some(x) => Ok(x),
            None => Err(todo!()),
        }?;
        //  3. Set this’s encoding to encoding.
        let this_encoding = encoding.whatwg_name().unwrap_or(encoding.name());

        //  4. If options["fatal"] is true, then set this’s error mode to "fatal".
        //  5. Set this’s ignore BOM to options["ignoreBOM"].
        Ok(TextDecoder {
            encoding: this_encoding.to_string(),
            fatal: (),
            ignoreBOM: (),
        })
    }

    // TODO: accessor!

    fn decode(
        // input: Option<allowsharedbufersource>,
        input: Option<()>,
        options: Option<TextDecodeOptions>,
    ) -> String {
        todo!()
    }
}
