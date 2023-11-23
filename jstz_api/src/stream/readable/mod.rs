use boa_engine::Context;
use jstz_core::native::register_global_class;

use self::{
    readable_byte_stream_controller::ReadableByteStreamControllerClass,
    readable_stream::ReadableStreamClass,
    readable_stream_byob_reader::ReadableStreamBYOBReaderClass,
    readable_stream_byob_request::ReadableStreamBYOBRequestClass,
    readable_stream_default_controller::ReadableStreamDefaultControllerClass,
    readable_stream_default_reader::ReadableStreamDefaultReaderClass,
};

pub mod internals;
pub mod readable_byte_stream_controller;
pub mod readable_stream;
pub mod readable_stream_byob_reader;
pub mod readable_stream_byob_request;
pub mod readable_stream_default_controller;
pub mod readable_stream_default_reader;

pub struct ReadableStreamApi;

impl jstz_core::Api for ReadableStreamApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<ReadableStreamClass>(context)
            .expect("The `ReadableStream` class shouldn't exist yet");
        register_global_class::<ReadableStreamDefaultReaderClass>(context)
            .expect("The `ReadableStreamDefaultReader` class shouldn't exist yet");
        register_global_class::<ReadableStreamBYOBReaderClass>(context)
            .expect("The `ReadableStreamBYOBReader` class shouldn't exist yet");
        register_global_class::<ReadableStreamDefaultControllerClass>(context)
            .expect("The `ReadableStreamDefaultController` class shouldn't exist yet");
        register_global_class::<ReadableByteStreamControllerClass>(context)
            .expect("The `ReadableByteStreamController` class shouldn't exist yet");
        register_global_class::<ReadableStreamBYOBRequestClass>(context)
            .expect("The `ReadableStreamBYOBRequest` class shouldn't exist yet");
    }
}
