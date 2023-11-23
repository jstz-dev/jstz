use boa_engine::{Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

use crate::idl;

use super::readable_stream_byob_request::ReadableStreamBYOBRequest;

// TODO

pub struct ReadableByteStreamController {}

impl Finalize for ReadableByteStreamController {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableByteStreamController {
    empty_trace!(); // TODO
}

impl ReadableByteStreamController {
    pub fn byob_request_getter() -> Option<ReadableStreamBYOBRequest> {
        todo!()
    }

    pub fn desired_result(&self) -> Option<idl::UnrestrictedDouble> {
        todo!()
    }
    // TODO
}

pub struct ReadableByteStreamControllerClass;

impl ReadableByteStreamControllerClass {
    // TODO
}

impl NativeClass for ReadableByteStreamControllerClass {
    type Instance = ReadableByteStreamController;

    const NAME: &'static str = "ReadableByteStreamController";

    fn constructor(
        this: &JsNativeObject<ReadableByteStreamController>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableByteStreamController> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
