// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date
/*
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

pub struct DateApi;
impl jstz_core::Api for DateApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<DateClass>(context)
            .expect("The `Date` class shouldn't exist yet");
    }
}
*/
