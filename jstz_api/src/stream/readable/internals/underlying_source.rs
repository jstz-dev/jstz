// https://streams.spec.whatwg.org/#underlying-source-api

use boa_engine::{
    object::{
        builtins::{JsFunction, JsPromise},
        NativeObject,
    },
    value::TryFromJs,
    Context, JsResult, JsValue,
};
use boa_gc::{empty_trace, Finalize, Trace};

use crate::{
    js_aware::JsOptional,
    stream::readable::{
        readable_byte_stream_controller::ReadableByteStreamController,
        readable_stream_default_controller::ReadableStreamDefaultController,
    },
};

// TODO also use the bytes version that is in deno?

#[derive(TryFromJs, Default)]
pub struct UnderlyingSource {
    pub start: JsOptional<UnderlyingSourceStartCallback>,
    pub pull: JsOptional<UnderlyingSourcePullCallback>,
    pub cancel: JsOptional<UnderlyingSourceCancelCallback>,
    pub r#type: JsOptional<ReadableStreamType>,
    pub auto_allocate_chunk_size: JsOptional<u64>, // TODO  [EnforceRange]
}

impl Finalize for UnderlyingSource {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for UnderlyingSource {
    empty_trace!(); // TODO
}

pub enum ReadableStreamController {
    DefaultController(ReadableStreamDefaultController),
    ByteController(ReadableByteStreamController),
}

impl Finalize for ReadableStreamController {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStreamController {
    empty_trace!(); // TODO
}

// pub type UnderlyingSourceStartCallback = fn(ReadableStreamController) -> JsValue;
// pub type UnderlyingSourcePullCallback = fn(ReadableStreamController) -> JsPromise;
// pub type UnderlyingSourceCancelCallback = fn(Option<JsValue>) -> JsPromise;
// TODO enforce types?
pub type UnderlyingSourceStartCallback = JsFunction;
pub type UnderlyingSourcePullCallback = JsFunction;
pub type UnderlyingSourceCancelCallback = JsFunction;

#[derive(PartialEq)]
pub enum ReadableStreamType {
    Bytes,
}

impl TryFromJs for ReadableStreamType {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        todo!()
    }
}
