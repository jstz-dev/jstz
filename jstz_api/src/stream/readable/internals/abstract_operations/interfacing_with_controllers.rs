// https://streams.spec.whatwg.org/#rs-abstract-ops-used-by-controllers

use std::mem;

use crate::{
    idl,
    js_aware::JsOptional,
    stream::readable::{
        internals::{
            readable_stream_generic_reader::AsReadableStreamGenericReader,
            types_for_readable_stream::{self, ReadableStreamState},
            types_for_readers::{ReadIntoRequest, ReadRequest},
        },
        readable_stream::ReadableStream,
        readable_stream_byob_reader::ReadableStreamBYOBReader,
    },
};
use boa_engine::{
    builtins::promise::Promise,
    object::{builtins::JsPromise, NativeObject},
    Context, JsError, JsResult, JsValue,
};
use jstz_core::runtime::with_global_host;

use crate::stream::readable::internals::types_for_readable_stream::*;
use crate::stream::readable::internals::types_for_readers::*;

pub fn readable_stream_add_read_into_request(
    stream: &mut ReadableStream,
    read_request: ReadIntoRequest,
) {
    // 1. Assert: stream.[[reader]] implements ReadableStreamBYOBReader.
    let reader = stream
        .reader
        .as_defined_mut()
        .unwrap()
        .as_byob_reader_mut()
        .unwrap();
    // 2. Assert: stream.[[state]] is "readable" or "closed".
    assert!(matches!(
        stream.state,
        ReadableStreamState::Readable | ReadableStreamState::Closed
    ));
    // 3. Append readRequest to stream.[[reader]].[[readIntoRequests]].
    reader.read_into_requests.push(read_request);
}

pub fn readable_stream_add_read_request(
    stream: &mut ReadableStream,
    read_request: ReadRequest,
) {
    // 1. Assert: stream.[[reader]] implements ReadableStreamDefaultReader.
    let reader = stream
        .reader
        .as_defined_mut()
        .unwrap()
        .as_default_reader_mut()
        .unwrap();
    // 2. Assert: stream.[[state]] is "readable".
    assert!(matches!(stream.state, ReadableStreamState::Readable));
    // 3. Append readRequest to stream.[[reader]].[[readRequests]].
    reader.read_requests.push(read_request);
}

pub fn readable_stream_cancel(
    stream: &mut ReadableStream,
    reason: idl::Any,
    context: &mut Context<'_>,
) -> JsResult<JsPromise> {
    // 1. Set stream.[[disturbed]] to true.
    stream.disturbed = true;
    // 2. If stream.[[state]] is "closed", return a promise resolved with undefined.
    if stream.state.is_closed() {
        return JsPromise::resolve(JsValue::undefined(), context);
    }
    // 3. If stream.[[state]] is "errored", return a promise rejected with stream.[[storedError]].
    if stream.state.is_errored() {
        return JsPromise::resolve(stream.stored_error.clone(), context);
    }
    // 4. Perform ! ReadableStreamClose(stream).
    // TODO handle !
    readable_stream_close(stream, context);
    // 5. Let reader be stream.[[reader]].
    let reader = &mut stream.reader;
    // 6. If reader is not undefined and reader implements ReadableStreamBYOBReader,
    if let Some(byob_reader) = reader
        .as_defined_mut()
        .and_then(ReadableStreamReader::as_byob_reader_mut)
    {
        // 1. Let readIntoRequests be reader.[[readIntoRequests]].
        // 2. Set reader.[[readIntoRequests]] to an empty list.
        // (1. and 2. are simultaneous)
        let read_into_requests =
            mem::replace(&mut byob_reader.read_into_requests, Vec::new());
        // 3. For each readIntoRequest of readIntoRequests,
        for read_into_request in read_into_requests {
            // 1. Perform readIntoRequest’s close steps, given undefined.
            read_into_request.close_steps(&JsValue::undefined(), context);
        }
    }
    todo!();
    // 7. Let sourceCancelPromise be ! stream.[[controller]].[[CancelSteps]](reason).
    // 8. Return the result of reacting to sourceCancelPromise with a fulfillment step that returns undefined.
}

pub fn readable_stream_close(stream: &mut ReadableStream, context: &mut Context<'_>) {
    // 1. Assert: stream.[[state]] is "readable".
    assert!(matches!(stream.state, ReadableStreamState::Readable));
    // 2. Set stream.[[state]] to "closed".
    stream.state = ReadableStreamState::Closed;
    // 3. Let reader be stream.[[reader]].
    let reader_or_undefined = &mut stream.reader;
    // 4. If reader is undefined, return.
    let JsOptional::Defined(reader) = reader_or_undefined else {
        return;
    };
    // 5. Resolve reader.[[closedPromise]] with undefined.
    // TODO I don't understand this. If the promise already exists, I can't just choose its value?
    // &mut reader.as_generic_reader_mut().closed_promise;
    // 6. If reader implements ReadableStreamDefaultReader,
    if let Some(default_reader) = reader.as_default_reader_mut() {
        // 1. Let readRequests be reader.[[readRequests]].
        // 2. Set reader.[[readRequests]] to an empty list.
        // (1. and 2. are simultaneous)
        let read_requests = mem::replace(&mut default_reader.read_requests, Vec::new());
        // 3. For each readRequest of readRequests,
        for read_request in read_requests {
            // 1. Perform readRequest’s close steps.
            read_request.close_steps(context);
        }
    }
}

pub fn readable_stream_error(
    stream: &mut ReadableStream,
    e: JsError,
    context: &mut Context<'_>,
) {
    // 1. Assert: stream.[[state]] is "readable".
    assert!(matches!(stream.state, ReadableStreamState::Readable));
    // 2. Set stream.[[state]] to "errored".
    stream.state = ReadableStreamState::Errored;
    // 3. Set stream.[[storedError]] to e.
    stream.stored_error = e.to_opaque(context);
    // 4. Let reader be stream.[[reader]].
    let reader_or_undefined = &mut stream.reader;
    // 5. If reader is undefined, return.
    let JsOptional::Defined(reader) = reader_or_undefined else {
        return;
    };
    let generic_reader = reader.as_generic_reader_mut();
    // 6. Reject reader.[[closedPromise]] with e.
    let promise = JsPromise::reject(e, context).unwrap(); // TODO handle Error case
    generic_reader.closed_promise = promise;
    // 7. Set reader.[[closedPromise]].[[PromiseIsHandled]] to true.
    // TODO
    // 8. If reader implements ReadableStreamDefaultReader,
    //    1. Perform ! ReadableStreamDefaultReaderErrorReadRequests(reader, e).
    // 9. Otherwise,
    //    1. Assert: reader implements ReadableStreamBYOBReader.
    //    2. Perform ! ReadableStreamBYOBReaderErrorReadIntoRequests(reader, e).
    // (8. and 9. handled by a match instead of an if-then-else)
    match reader {
        ReadableStreamReader::DefaultReader(_) => {
            todo!()
        }
        ReadableStreamReader::BYOBReader(_) => {
            todo!()
        }
    }
}

pub fn readable_stream_fulfill_read_into_request(
    stream: &mut ReadableStream,
    chunk: idl::Chunk,
    done: bool,
    context: &mut Context<'_>,
) {
    // 1. Assert: ! ReadableStreamHasBYOBReader(stream) is true.
    assert!(readable_stream_has_byob_reader(stream));
    // 2. Let reader be stream.[[reader]].
    let reader = &mut stream
        .reader
        .as_defined_mut()
        .unwrap()
        .as_byob_reader_mut()
        .unwrap();
    // TODO handle errors
    // 3. Assert: reader.[[readIntoRequests]] is not empty.
    // 4. Let readIntoRequest be reader.[[readIntoRequests]][0].
    // 5. Remove readIntoRequest from reader.[[readIntoRequests]].
    let Some(read_into_request) = reader.read_into_requests.pop() else {
        panic!(); // TODO better error handling
    };
    // 6. If done is true, perform readIntoRequest’s close steps, given chunk.
    // 7. Otherwise, perform readIntoRequest’s chunk steps, given chunk.
    if done {
        read_into_request.close_steps(&chunk, context); // TODO & not needed?
    } else {
        read_into_request.chunk_steps(&chunk, context); // TODO & not needed?
    }
}

pub fn readable_stream_fulfill_read_request(
    stream: &mut ReadableStream,
    chunk: idl::Chunk,
    done: bool,
    context: &mut Context<'_>,
) {
    // 1. Assert: ! ReadableStreamHasDefaultReader(stream) is true.
    assert!(readable_stream_has_default_reader(stream));
    // 2. Let reader be stream.[[reader]].
    let reader = &mut stream
        .reader
        .as_defined_mut()
        .unwrap()
        .as_default_reader_mut()
        .unwrap();
    // TODO handle error
    // 3. Assert: reader.[[readRequests]] is not empty.
    // 4. Let readRequest be reader.[[readRequests]][0].
    // 5. Remove readRequest from reader.[[readRequests]].
    let Some(read_request) = reader.read_requests.pop() else {
        panic!(); // TODO better error handling
    };
    // 6. If done is true, perform readRequest’s close steps.
    // 7. Otherwise, perform readRequest’s chunk steps, given chunk.
    if done {
        read_request.close_steps(context);
    } else {
        read_request.chunk_steps(&chunk, context); // TODO & not needed?
    }
}

pub fn readable_stream_get_num_read_into_requests(
    stream: &ReadableStream,
) -> idl::Number {
    // 1. Assert: ! ReadableStreamHasBYOBReader(stream) is true.
    assert!(readable_stream_has_byob_reader(stream));
    // 2. Return stream.[[reader]].[[readIntoRequests]]'s size.
    // TODO handle errors
    return stream
        .reader
        .as_defined()
        .unwrap()
        .as_byob_reader()
        .unwrap()
        .read_into_requests
        .len() as idl::Number;
}

pub fn readable_stream_get_num_read_requests(stream: &ReadableStream) -> idl::Number {
    // 1. Assert: ! ReadableStreamHasDefaultReader(stream) is true.
    assert!(readable_stream_has_default_reader(stream));
    // 2. Return stream.[[reader]].[[readRequests]]'s size.
    stream
        .reader
        .as_defined()
        .unwrap()
        .as_default_reader()
        .unwrap() // TODO handle error
        .read_requests
        .len() as idl::Number
    // TODO handle errors
}

pub fn readable_stream_has_byob_reader(stream: &ReadableStream) -> bool {
    // 1. Let reader be stream.[[reader]].
    let reader_or_undefined = &stream.reader;
    // 2. If reader is undefined, return false.
    // 3. If reader implements ReadableStreamBYOBReader, return true.
    // 4. Return false.
    // (2., 3., and 4. handled by a match instead of if-then-else-s)
    match reader_or_undefined {
        JsOptional::Undefined => false,
        JsOptional::Defined(ReadableStreamReader::BYOBReader(_)) => true,
        _ => false,
    }
}

pub fn readable_stream_has_default_reader(stream: &ReadableStream) -> bool {
    // 1. Let reader be stream.[[reader]].
    let reader_or_undefined = &stream.reader;
    // 2. If reader is undefined, return false.
    // 3. If reader implements ReadableStreamDefaultReader, return true.
    // 4. Return false.
    // (2., 3., and 4. handled by a match instead of if-then-else-s)
    match reader_or_undefined {
        JsOptional::Undefined => false,
        JsOptional::Defined(ReadableStreamReader::DefaultReader(_)) => true,
        _ => false,
    }
}
