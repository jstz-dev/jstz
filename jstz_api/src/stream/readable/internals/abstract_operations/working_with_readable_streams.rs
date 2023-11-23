use boa_engine::JsResult;

use crate::{
    idl,
    js_aware::*,
    stream::readable::{
        internals::types_for_readable_stream::ReadableStreamState,
        readable_stream::ReadableStream,
        readable_stream_byob_reader::ReadableStreamBYOBReader,
        readable_stream_default_reader::ReadableStreamDefaultReader,
    },
    tmp::Todo,
};

// AcquireReadableStreamBYOBReader(stream)
pub fn acquire_readable_stream_byob_reader(
    stream: &mut ReadableStream,
) -> JsResult<ReadableStreamBYOBReader> {
    // 1. Let reader be a new ReadableStreamBYOBReader.

    // 2. Perform ? SetUpReadableStreamBYOBReader(reader, stream).

    // 3. Return reader.
    todo!()
}

// AcquireReadableStreamDefaultReader(stream)
pub fn acquire_readable_stream_default_reader(
    stream: &mut ReadableStream,
) -> JsResult<ReadableStreamDefaultReader> {
    // 1. Let reader be a new ReadableStreamDefaultReader.

    // 2. Perform ? SetUpReadableStreamDefaultReader(reader, stream).

    // 3. Return reader.
    todo!()
}

// CreateReadableStream(startAlgorithm, pullAlgorithm, cancelAlgorithm[, highWaterMark, [, sizeAlgorithm]])
// https://streams.spec.whatwg.org/index.html#create-readable-stream
pub fn create_readable_stream(
    startAlgorithm: Todo,
    pullAlgorithm: Todo,
    cancelAlgorithm: Todo,
    highWaterMark: Option<Todo>,
    sizeAlgorithm: Option<Todo>,
) {
    // 1. If highWaterMark was not passed, set it to 1.

    // 2. If sizeAlgorithm was not passed, set it to an algorithm that returns 1.

    // 3. Assert: ! IsNonNegativeNumber(highWaterMark) is true.

    // 4. Let stream be a new ReadableStream.

    // 5. Perform ! InitializeReadableStream(stream).

    // 6. Let controller be a new ReadableStreamDefaultController.

    // 7. Perform ? SetUpReadableStreamDefaultController(stream, controller, startAlgorithm, pullAlgorithm, cancelAlgorithm, highWaterMark, sizeAlgorithm).

    // 8. Return stream.
}

// CreateReadableByteStream(startAlgorithm, pullAlgorithm, cancelAlgorithm)
// Note: This abstract operation will throw an exception if and only if the supplied startAlgorithm throws.
// https://streams.spec.whatwg.org/#abstract-opdef-createreadablebytestream
pub fn create_readable_byte_stream(
    startAlgorithm: Todo,
    pullAlgorithm: Todo,
    cancelAlgorithm: Todo,
) {
    // 1. Let stream be a new ReadableStream.

    // 2. Perform ! InitializeReadableStream(stream).

    // 3. Let controller be a new ReadableByteStreamController.

    // 4. Perform ? SetUpReadableByteStreamController(stream, controller, startAlgorithm, pullAlgorithm, cancelAlgorithm, 0, undefined).

    // 5. Return stream.

    todo!()
}

// InitializeReadableStream(stream)
// https://streams.spec.whatwg.org/#initialize-readable-stream
pub fn initialize_readable_stream(stream: &mut ReadableStream) {
    // 1. Set stream.[[state]] to "readable".
    stream.state = ReadableStreamState::Readable;
    // 2. Set stream.[[reader]] and stream.[[storedError]] to undefined.
    stream.reader = todo!();
    stream.stored_error = todo!();
    // 3. Set stream.[[disturbed]] to false.
    stream.disturbed = false;
}

// IsReadableStreamLocked(stream)
// https://streams.spec.whatwg.org/#is-readable-stream-locked
pub fn is_readable_stream_locked(stream: &ReadableStream) -> bool {
    // 1. If stream.[[reader]] is undefined, return false.
    // 2. Return true.
    return !(stream.reader.is_undefined());
}

// ReadableStreamFromIterable(asyncIterable)
// https://streams.spec.whatwg.org/#readable-stream-from-iterable
pub fn readable_stream_from_iterable(asyncIterable: Todo) -> JsResult<ReadableStream> {
    todo!()
}

// ReadableStreamPipeTo(source, dest, preventClose, preventAbort, preventCancel[, signal])
// https://streams.spec.whatwg.org/#readable-stream-pipe-to
pub fn readable_stream_pipe_to(
    source: Todo,
    dest: Todo,
    preventClose: Todo,
    preventAbort: Todo,
    preventCancel: Todo,
    signal: Option<Todo>,
) {
    todo!()
}

// ReadableStreamTee(stream, cloneForBranch2)
// https://streams.spec.whatwg.org/#readable-stream-tee
pub fn readable_stream_tee(
    stream: &mut ReadableStream,
    cloneForBranch2: bool,
) -> JsResult<(ReadableStream, ReadableStream)> {
    // 1. Assert: stream implements ReadableStream.

    // 2. Assert: cloneForBranch2 is a boolean.

    // 3. If stream.[[controller]] implements ReadableByteStreamController, return ? ReadableByteStreamTee(stream).

    // 4. Return ? ReadableStreamDefaultTee(stream, cloneForBranch2).

    todo!()
}

// ReadableStreamDefaultTee(stream, cloneForBranch2)
// https://streams.spec.whatwg.org/#abstract-opdef-readablestreamdefaulttee
pub fn readable_stream_default_tee(stream: &ReadableStream, cloneForBranch2: Todo) {
    todo!()
}

// ReadableByteStreamTee(stream)
// https://streams.spec.whatwg.org/#abstract-opdef-readablebytestreamtee
pub fn readable_byte_stream_tee(stream: &ReadableStream) {
    todo!()
}
