use boa_engine::{object::builtins::JsPromise, Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

use super::{
    internals::{
        readable_stream_generic_reader::{
            ReadableStreamGenericReader, ReadableStreamGenericReaderTrait,
        },
        types_for_readers::ReadRequest,
    },
    readable_stream::ReadableStream,
};

// https://streams.spec.whatwg.org/#default-reader-class

// TODO

#[derive(Clone)]
pub struct ReadableStreamDefaultReader {
    pub generic_reader: ReadableStreamGenericReader,
    pub read_requests: Vec<ReadRequest>,
}

impl Finalize for ReadableStreamDefaultReader {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStreamDefaultReader {
    empty_trace!(); // TODO
}

impl ReadableStreamDefaultReader {
    pub fn new(stream: ReadableStream) -> JsResult<Self> {
        todo!()
    }

    pub fn read() -> JsPromise /* of ReadableStreamReadResult */ {
        todo!()
    }

    pub fn release_lock() -> () {
        todo!()
    }
}

impl ReadableStreamGenericReaderTrait for ReadableStreamDefaultReader {
    fn closed_promise_getter(&self) -> JsPromise {
        return self.generic_reader.closed_promise_getter();
    }

    fn cancel(&self, reason: Option<JsValue>) -> JsPromise {
        return self.generic_reader.cancel(reason);
    }
}

pub struct ReadableStreamDefaultReaderClass;

impl ReadableStreamDefaultReaderClass {
    // TODO
}

impl NativeClass for ReadableStreamDefaultReaderClass {
    type Instance = ReadableStreamDefaultReader;

    const NAME: &'static str = "ReadableStreamDefaultReader";

    fn constructor(
        this: &JsNativeObject<ReadableStreamDefaultReader>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableStreamDefaultReader> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
