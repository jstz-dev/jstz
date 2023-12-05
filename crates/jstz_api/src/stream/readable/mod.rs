use boa_engine::{Context, JsResult};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::native::{
    register_global_class, ClassBuilder, JsNativeObject, NativeClass,
};

pub struct ReadableStream {
    // TODO
}

impl Finalize for ReadableStream {
    fn finalize(&self) {
        todo!()
    }
}

unsafe impl Trace for ReadableStream {
    custom_trace!(this, todo!());
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
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
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
