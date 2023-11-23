use crate::idl;
use crate::js_aware::*;
use crate::stream::readable::internals::abstract_operations::working_with_readable_streams::*;
use crate::stream::readable::internals::underlying_source::ReadableStreamType;
use crate::stream::strategy::queuing_strategy::QueuingStrategy;
use crate::tmp::Todo;

use super::super::writable::writable_stream::WritableStream;

use super::internals::abstract_operations::interfacing_with_controllers::readable_stream_cancel;
use super::internals::types_for_readable_stream::*;
use super::internals::underlying_source::ReadableStreamController;
use super::internals::underlying_source::UnderlyingSource;
use boa_engine::value::TryFromJs;
use boa_engine::{object::builtins::JsPromise, Context, JsObject, JsResult, JsValue};
use boa_engine::{JsError, JsNativeError};
use boa_gc::GcRef;
use boa_gc::GcRefMut;
use boa_gc::{empty_trace, Finalize, Trace};
use core::clone::Clone;
use core::ops::Deref;
use jstz_core::native::{ClassBuilder, JsNativeObject, NativeClass};
use std::convert::Infallible;
use std::ops::DerefMut;
// TODO

pub struct ReadableStream {
    /// A ReadableStreamDefaultController or ReadableByteStreamController created with the ability to control the state and queue of this stream
    pub controller: JsOptional<JsNativeObject<ReadableStreamController>>,
    /// A boolean flag set to true when the stream is transferred
    pub detached: bool,
    /// A boolean flag set to true when the stream has been read from or canceled
    pub disturbed: bool,
    /// A ReadableStreamDefaultReader or ReadableStreamBYOBReader instance, if the stream is locked to a reader, or undefined if it is not
    pub reader: JsOptional<ReadableStreamReader>,
    /// A string containing the stream’s current state, used internally; one of "readable", "closed", or "errored"
    pub state: ReadableStreamState,
    /// A value indicating how the stream failed, to be given as a failure reason or exception when trying to operate on an errored stream
    pub stored_error: JsValue,
}

impl Finalize for ReadableStream {
    fn finalize(&self) {
        todo!();
    }
}

unsafe impl Trace for ReadableStream {
    empty_trace!(); // TODO
}

impl ReadableStream {
    /// [constructor][spec](optional object underlyingSource , optional QueuingStrategy strategy  = {});
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-constructor
    // The new ReadableStream(underlyingSource, strategy) constructor steps are:
    pub fn new(
        this: JsNativeObject<Self>,
        underlying_source_or_null_or_undefined: JsOptional<
            JsNullable<JsNativeObject<UnderlyingSource>>, // TODO en fait c'est bien JsObject?)
        >,
        strategy: JsOptional<QueuingStrategy>,
    ) -> JsResult<Self> {
        // 1. If underlyingSource is missing, set it to null.

        // 2. Let underlyingSourceDict be underlyingSource, converted to an IDL value of type UnderlyingSource.

        // TODO handle args
        let underlying_source_dict = UnderlyingSource::default();
        // 3. Perform ! InitializeReadableStream(this).
        let mut this_gc_cell = this.deref_mut();
        initialize_readable_stream(this_gc_cell.deref_mut());
        // 4. If underlyingSourceDict["type"] is "bytes":
        if underlying_source_dict.r#type == JsOptional::Defined(ReadableStreamType::Bytes)
        {
            // TODO
            // 1. If strategy["size"] exists, throw a RangeError exception.

            // 2. Let highWaterMark be ? ExtractHighWaterMark(strategy, 0).

            // 3. Perform ? SetUpReadableByteStreamControllerFromUnderlyingSource(this, underlyingSource, underlyingSourceDict, highWaterMark).
        }
        // 5. Otherwise,
        else {
            // 1. Assert: underlyingSourceDict["type"] does not exist.
            assert!(underlying_source_dict.r#type == JsOptional::Undefined);
            // 2. Let sizeAlgorithm be ! ExtractSizeAlgorithm(strategy).
            // TODO
            // 3. Let highWaterMark be ? ExtractHighWaterMark(strategy, 1).

            // 4. Perform ? SetUpReadableStreamDefaultControllerFromUnderlyingSource(this, underlyingSource, underlyingSourceDict, highWaterMark, sizeAlgorithm).
        }
        let x = Self {
            controller: todo!(),
            detached: todo!(),
            disturbed: todo!(),
            reader: todo!(),
            state: todo!(),
            stored_error: todo!(),
        };
        Ok(x)
    }

    /// readonly attribute boolean [locked][spec];
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-locked
    //
    // The locked getter steps are:
    pub fn locked_getter(self: &Self) -> bool {
        // 1. Return ! IsReadableStreamLocked(this).
        is_readable_stream_locked(self)
    }

    /// Promise<undefined> [cancel][spec](optional any reason);
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-cancel
    // The cancel(reason) method steps are:
    pub fn cancel(
        self: &mut Self,
        // TODO change to "reason: idl::Any"?
        reason: JsOptional<idl::Any>,
        context: &mut Context<'_>,
    ) -> JsResult<JsPromise> // TODO should this return a JsPromise and handle errors internally?
    {
        // 1. If ! IsReadableStreamLocked(this) is true, return a promise rejected with a TypeError exception.
        if is_readable_stream_locked(self) {
            return JsPromise::reject(JsNativeError::typ(), context);
        }
        // 2. Return ! ReadableStreamCancel(this, reason).
        return readable_stream_cancel(self, reason.into(), context);
    }

    /// ReadableStreamReader [getReader][spec](optional ReadableStreamGetReaderOptions options = {});
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-get-reader
    // The getReader(options) method steps are:
    pub fn get_reader(
        self: &mut Self,
        options_or_undefined: JsOptional<ReadableStreamGetReaderOptions>,
    ) -> JsResult<ReadableStreamReader> {
        let options = options_or_undefined.into_defined().unwrap_or_default();
        // 1. If options["mode"] does not exist, return ? AcquireReadableStreamDefaultReader(this).
        if options.mode.is_undefined() {
            return acquire_readable_stream_default_reader(self)
                .map(ReadableStreamReader::DefaultReader);
        }
        // 2. Assert: options["mode"] is "byob".
        assert!(matches!(
            options.mode,
            JsOptional::Defined(ReadableStreamReaderMode::BYOB)
        ));
        // 3. Return ? AcquireReadableStreamBYOBReader(this).
        return acquire_readable_stream_byob_reader(self)
            .map(ReadableStreamReader::BYOBReader);
    }

    /// ReadableStream [pipeThrough][spec](ReadableWritablePair transform, optional StreamPipeOptions options = {});
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-pipe-through
    // The pipeThrough(transform, options) method steps are:
    pub fn pipe_through(
        transform: ReadableWritablePair,
        options_or_undefined: JsOptional<StreamPipeOptions>,
    ) {
        let options = options_or_undefined.into_defined().unwrap_or_default();
        // 1. If ! IsReadableStreamLocked(this) is true, throw a TypeError exception.

        // 2. If ! IsWritableStreamLocked(transform["writable"]) is true, throw a TypeError exception.

        // 3. Let signal be options["signal"] if it exists, or undefined otherwise.

        // 4. Let promise be ! ReadableStreamPipeTo(this, transform["writable"], options["preventClose"], options["preventAbort"], options["preventCancel"], signal).

        // 5. Set promise.[[PromiseIsHandled]] to true.

        // 6. Return transform["readable"].

        todo!()
    }

    /// Promise<undefined> [pipeTo][spec](WritableStream destination, optional StreamPipeOptions options  = {});
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-pipe-to
    // The pipeTo(destination, options) method steps are:
    pub fn pipe_to(
        destination: WritableStream,
        options_or_undefined: JsOptional<StreamPipeOptions>,
    ) {
        let options = options_or_undefined.into_defined().unwrap_or_default();
        // 1. If ! IsReadableStreamLocked(this) is true, return a promise rejected with a TypeError exception.

        // 2. If ! IsWritableStreamLocked(destination) is true, return a promise rejected with a TypeError exception.

        // 3. Let signal be options["signal"] if it exists, or undefined otherwise.

        // 4. Return ! ReadableStreamPipeTo(this, destination, options["preventClose"], options["preventAbort"], options["preventCancel"], signal).

        todo!()
    }

    /// sequence<ReadableStream> [tee][spec]();
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-tee
    /// (The spec requires this method to always return exactly two ReadableStream, which is why the Rust return type is a pair instead of a sequence)
    // The tee() method steps are:
    pub fn tee(&mut self) -> JsResult<(ReadableStream, ReadableStream)> {
        // 1. Return ? ReadableStreamTee(this, false).
        return readable_stream_tee(self, false);
    }

    // async iterable<any>(optional ReadableStreamIteratorOptions options  = {});
    // TODO
}

pub struct ReadableStreamBuilder {}

impl ReadableStreamBuilder {
    /// static ReadableStream [from][spec](any asyncIterable);
    ///
    /// [spec]: https://streams.spec.whatwg.org/#rs-from
    // The static from(asyncIterable) method steps are:
    pub fn from(async_iterable: Todo) -> JsResult<ReadableStream> {
        // 1. Return ? ReadableStreamFromIterable(asyncIterable).
        readable_stream_from_iterable(async_iterable)
    }
}

pub struct ReadableStreamClass;

impl ReadableStreamClass {
    // TODO
}

impl NativeClass for ReadableStreamClass {
    type Instance = ReadableStream;

    const NAME: &'static str = "ReadableStream";

    fn constructor(
        this: &JsNativeObject<ReadableStream>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<ReadableStream> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        // TODO
        Ok(())
    }
}
