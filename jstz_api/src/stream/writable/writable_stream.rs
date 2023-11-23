use boa_engine::{Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

pub struct WritableStream {
    // TODO
}

impl Finalize for WritableStream {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for WritableStream {
    empty_trace!(); // TODO
}

impl WritableStream {
    // TODO
}

pub struct WritableStreamClass;

impl WritableStreamClass {
    // TODO
}

impl NativeClass for WritableStreamClass {
    type Instance = WritableStream;

    const NAME: &'static str = "WritableStream";

    fn constructor(
        this: &JsNativeObject<WritableStream>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<WritableStream> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
