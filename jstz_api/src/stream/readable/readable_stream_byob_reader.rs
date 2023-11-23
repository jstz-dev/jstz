use boa_engine::{object::builtins::JsPromise, Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

use crate::idl;

use super::{
    internals::{
        readable_stream_generic_reader::{
            ReadableStreamGenericReader, ReadableStreamGenericReaderTrait,
        },
        types_for_readers::ReadIntoRequest,
    },
    readable_stream::ReadableStream,
};

// TODO
#[derive(Clone)]
pub struct ReadableStreamBYOBReader {
    pub generic_reader: ReadableStreamGenericReader,
    pub read_into_requests: Vec<ReadIntoRequest>,
}

impl Finalize for ReadableStreamBYOBReader {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStreamBYOBReader {
    empty_trace!(); // TODO
}

impl ReadableStreamBYOBReader {
    pub fn new(stream: ReadableStream) -> JsResult<Self> {
        todo!()
    }

    pub fn read(view: idl::JsArrayBufferView) -> JsPromise /* of ReadableStreamReadResult */
    {
        todo!()
    }

    pub fn release_lock() {
        todo!()
    }
}

impl ReadableStreamGenericReaderTrait for ReadableStreamBYOBReader {
    fn closed_promise_getter(&self) -> JsPromise {
        return self.generic_reader.closed_promise_getter();
    }

    fn cancel(&self, reason: Option<JsValue>) -> JsPromise {
        return self.generic_reader.cancel(reason);
    }
}

pub struct ReadableStreamBYOBReaderClass;

impl ReadableStreamBYOBReaderClass {
    // TODO
}

impl NativeClass for ReadableStreamBYOBReaderClass {
    type Instance = ReadableStreamBYOBReader;

    const NAME: &'static str = "ReadableStreamBYOBReader";

    fn constructor(
        this: &JsNativeObject<ReadableStreamBYOBReader>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableStreamBYOBReader> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
