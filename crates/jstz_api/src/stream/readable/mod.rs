use boa_engine::{value::TryFromJs, Context, JsArgs, JsResult};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::native::{
    register_global_class, ClassBuilder, JsNativeObject, NativeClass,
};

use crate::stream::readable::underlying_source::UnderlyingSource;

pub mod underlying_source;

pub struct ReadableStream {
    // TODO
}

impl Finalize for ReadableStream {
    fn finalize(&self) {
        todo!()
    }
}

unsafe impl Trace for ReadableStream {
    custom_trace!(this, {
        let _ = this;
        todo!()
    });
}

pub struct ReadableStreamClass;

impl NativeClass for ReadableStreamClass {
    type Instance = ReadableStream;

    const NAME: &'static str = "ReadableStream";

    fn constructor(
        _this: &JsNativeObject<Self::Instance>,
        args: &[boa_engine::JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Self::Instance> {
        let underlying_source =
            Option::<UnderlyingSource>::try_from_js(args.get_or_undefined(0), context)?;
        let _ = underlying_source;
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        let _ = class;
        Ok(())
    }
}

pub struct ReadableStreamApi;

impl jstz_core::Api for ReadableStreamApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<ReadableStreamClass>(context)
            .expect("The `ReadableStream` class shouldn't exist yet")
        // TODO
    }
}
