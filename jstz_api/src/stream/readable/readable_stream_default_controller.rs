use crate::js_aware::JsNullable;
use boa_engine::{object::builtins::JsPromise, Context, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize, Trace};
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};

use crate::{
    idl,
    stream::readable::internals::abstract_operations::default_controllers::readable_stream_default_controller_can_close_or_enqueue,
    tmp::Todo,
};

use super::{
    internals::abstract_operations::default_controllers::*,
    readable_stream::ReadableStream,
    readable_stream_byob_request::ReadableStreamBYOBRequest,
};

// TODO

// https://streams.spec.whatwg.org/#rbs-controller-class

pub struct ReadableStreamDefaultController {
    /// A promise-returning algorithm, taking one argument (the cancel reason), which communicates a requested cancelation to the underlying source
    pub cancel_algorithm: Option<Box<dyn Fn(idl::Any) -> JsPromise>>,
    /// A boolean flag indicating whether the stream has been closed by its underlying source, but still has chunks in its internal queue that have not yet been read
    pub close_requested: bool,
    /// A boolean flag set to true if the stream’s mechanisms requested a call to the underlying source's pull algorithm to pull more data, but the pull could not yet be done since a previous call is still executing
    pub pull_again: bool,
    /// A promise-returning algorithm that pulls data from the underlying source
    pub pull_algorithm: Todo,
    /// A boolean flag set to true while the underlying source's pull algorithm is executing and the returned promise has not yet fulfilled, used to prevent reentrant calls
    pub pulling: bool,
    /// A list representing the stream’s internal queue of chunks
    pub queue: Vec<idl::Chunk>,
    /// The total size of all the chunks stored in [[queue]] (see § 8.1 Queue-with-sizes)
    pub queue_total_size: idl::Number,
    /// A boolean flag indicating whether the underlying source has finished starting
    pub started: bool,
    /// A number supplied to the constructor as part of the stream’s queuing strategy, indicating the point at which the stream will apply backpressure to its underlying source
    pub strategy_hwm: idl::Number,
    /// An algorithm to calculate the size of enqueued chunks, as part of the stream’s queuing strategy
    pub strategy_size_algorithm: Todo,
    /// The ReadableStream instance controlled
    pub stream: JsNativeObject<ReadableStream>,
}

impl Finalize for ReadableStreamDefaultController {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStreamDefaultController {
    empty_trace!(); // TODO
}

impl ReadableStreamDefaultController {
    // The desiredSize getter steps are:
    pub fn desired_size_getter(&self) -> JsNullable<idl::UnrestrictedDouble> {
        // 1. Return ! ReadableStreamDefaultControllerGetDesiredSize(this).
        return readable_stream_default_controller_get_desired_size(self);
    }

    // The close() method steps are:
    pub fn close(&mut self, context: &mut Context<'_>) -> JsResult<Todo> {
        // 1. If ! ReadableStreamDefaultControllerCanCloseOrEnqueue(this) is false, throw a TypeError exception.
        if readable_stream_default_controller_can_close_or_enqueue(self) == false {
            panic!(); // TODO use JavaScript Error
        }
        // 2. Perform ! ReadableStreamDefaultControllerClose(this).
        readable_stream_default_controller_close(self, context); // Todo Check that is can not throw exception
        todo!()
        //Ok(JsOptional::Undefined)
    }

    pub fn enqueue(&self, chunk: idl::JsArrayBufferView) {
        todo!()
    }

    pub fn error(&self, e: Option<idl::Any>) {
        todo!()
    }
    // TODO
}

pub struct ReadableStreamDefaultControllerClass;

impl ReadableStreamDefaultControllerClass {
    // TODO
}

impl NativeClass for ReadableStreamDefaultControllerClass {
    type Instance = ReadableStreamDefaultController;

    const NAME: &'static str = "ReadableStreamDefaultController";

    fn constructor(
        this: &JsNativeObject<ReadableStreamDefaultController>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableStreamDefaultController> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
