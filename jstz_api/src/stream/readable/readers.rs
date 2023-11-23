use std::collections::LinkedList;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, Object},
    property::Attribute,
    Context, JsNativeError, JsResult, JsValue,
};
use boa_gc::{custom_trace, empty_trace, Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};

use super::readable_stream::ReadableStream;

pub struct ReadRequest {
    // TODO
}

pub struct ReadableStreamDefaultReader {
    generic_reader: ReadableStreamGenericReader,
    read_requests: LinkedList<ReadRequest>, // TODO really use LinkedList?
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
    pub fn closed(&self) -> JsPromise {
        todo!()
    }

    pub fn new(stream: &ReadableStream) -> JsResult<Self> {
        todo!()
    }

    pub fn read(&self) -> JsPromise {
        todo!()
    }

    pub fn release_lock(&self) {
        todo!()
    }

    pub fn cancel(&self) -> JsPromise {
        todo!()
    }

    pub fn cancel_with_reason(&self, reason: &JsValue) -> JsPromise {
        todo!()
    }
}

pub struct ReadableStreamDefaultReaderClass;

impl ReadableStreamDefaultReader {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `ReadableStreamDefaultReader`")
                    .into()
            })
    }
}
impl ReadableStreamDefaultReaderClass {
    fn closed(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            ReadableStreamDefaultReader,
            "closed",
            get:((stream_reader, context) => Ok(stream_reader.closed().into_js(context)))
        )
    }

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
        let closed = ReadableStreamDefaultReaderClass::closed(class.context());
        class.accessor(js_string!("closed"), closed, Attribute::default());

        Ok(())
    }
}

pub struct ReadableStreamDefaultReaderApi;

impl jstz_core::Api for ReadableStreamDefaultReaderApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<ReadableStreamDefaultReaderClass>(context)
            .expect("The `ReadableStreamDefaultReader` class shouldn't exist yet")
    }
}
