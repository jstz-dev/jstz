use boa_engine::{Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

// TODO

pub struct ReadableStreamBYOBRequest {
    // TODO
}

impl Finalize for ReadableStreamBYOBRequest {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStreamBYOBRequest {
    empty_trace!(); // TODO
}

impl ReadableStreamBYOBRequest {
    // TODO
}

pub struct ReadableStreamBYOBRequestClass;

impl ReadableStreamBYOBRequestClass {
    // TODO
}

impl NativeClass for ReadableStreamBYOBRequestClass {
    type Instance = ReadableStreamBYOBRequest;

    const NAME: &'static str = "ReadableStreamBYOBRequest";

    fn constructor(
        this: &JsNativeObject<ReadableStreamBYOBRequest>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableStreamBYOBRequest> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
