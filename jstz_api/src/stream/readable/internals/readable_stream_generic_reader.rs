use boa_engine::{object::builtins::JsPromise, JsValue};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::native::JsNativeObject;

use crate::stream::readable::{
    readable_stream_byob_reader::ReadableStreamBYOBReader,
    readable_stream_default_reader::ReadableStreamDefaultReader,
};

use super::{
    super::readable_stream::ReadableStream,
    types_for_readable_stream::ReadableStreamReader,
};

// https://streams.spec.whatwg.org/#generic-reader-mixin

#[derive(Clone)]
pub struct ReadableStreamGenericReader {
    pub closed_promise: JsPromise,
    pub stream: JsNativeObject<ReadableStream>,
}

impl Finalize for ReadableStreamGenericReader {
    fn finalize(&self) {
        todo!()
        //self.closed_promise.finalize();
        //self.stream.finalize();
    }
}

unsafe impl Trace for ReadableStreamGenericReader {
    // TODO check
    custom_trace!(this, {
        mark(&this.closed_promise);
        mark(&this.stream);
    });
}

pub trait AsReadableStreamGenericReader {
    fn as_generic_reader_mut(&mut self) -> &mut ReadableStreamGenericReader;
}

impl AsReadableStreamGenericReader for ReadableStreamDefaultReader {
    fn as_generic_reader_mut(&mut self) -> &mut ReadableStreamGenericReader {
        return &mut self.generic_reader;
    }
}

impl AsReadableStreamGenericReader for ReadableStreamBYOBReader {
    fn as_generic_reader_mut(&mut self) -> &mut ReadableStreamGenericReader {
        return &mut self.generic_reader;
    }
}

impl AsReadableStreamGenericReader for ReadableStreamReader {
    fn as_generic_reader_mut(&mut self) -> &mut ReadableStreamGenericReader {
        match self {
            ReadableStreamReader::DefaultReader(reader) => reader.as_generic_reader_mut(),
            ReadableStreamReader::BYOBReader(reader) => reader.as_generic_reader_mut(),
        }
    }
}

pub trait ReadableStreamGenericReaderTrait {
    fn closed_promise_getter(&self) -> JsPromise;

    fn cancel(&self, reason: Option<JsValue>) -> JsPromise;
}

impl ReadableStreamGenericReaderTrait for ReadableStreamGenericReader {
    fn closed_promise_getter(&self) -> JsPromise {
        todo!()
        //return self.closed_promise.clone();
    }

    fn cancel(&self, reason: Option<JsValue>) -> JsPromise {
        todo!()
    }
}
