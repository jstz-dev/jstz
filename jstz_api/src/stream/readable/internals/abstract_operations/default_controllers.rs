use crate::idl::*;
use crate::js_aware::JsNullable;
use crate::stream::readable::internals::types_for_readable_stream::ReadableStreamState;
use crate::tmp::*;
use crate::{
    idl,
    stream::readable::{
        internals::underlying_source::UnderlyingSource, readable_stream::ReadableStream,
        readable_stream_default_controller::ReadableStreamDefaultController,
    },
};
use boa_engine::object::Object;
use boa_engine::{Context, JsError};
use boa_gc::GcRef;
use boa_gc::GcRefMut;
use core::clone::Clone;
use core::ops::Deref;
use core::ops::DerefMut;

use super::interfacing_with_controllers::*;

// https://streams.spec.whatwg.org/#rs-default-controller-abstract-ops
// The following abstract operations support the implementation of the ReadableStreamDefaultController class.

// https://streams.spec.whatwg.org/#readable-stream-default-controller-call-pull-if-needed
// ReadableStreamDefaultControllerCallPullIfNeeded(controller) performs the following steps:
pub fn readable_stream_default_controller_call_pull_if_needed(
    controller: &mut ReadableStreamDefaultController,
) {
    // 1. Let shouldPull be ! ReadableStreamDefaultControllerShouldCallPull(controller).
    let should_pull = readable_stream_default_controller_should_call_pull(controller);
    // 2. If shouldPull is false, return.
    if should_pull {
        return;
    }
    // 3. If controller.[[pulling]] is true,
    if controller.pulling {
        // 1.Set controller.[[pullAgain]] to true.
        controller.pull_again = true;
        // 2. Return.
        return;
    }
    // 4. Assert: controller.[[pullAgain]] is false.
    assert!(controller.pull_again == false);
    // 5. Set controller.[[pulling]] to true.
    controller.pulling = true;
    // 6. Let pullPromise be the result of performing controller.[[pullAlgorithm]].

    // 7. Upon fulfillment of pullPromise,
    {
        // 1. Set controller.[[pulling]] to false.

        // 2. If controller.[[pullAgain]] is true,
        {
            // 1. Set controller.[[pullAgain]] to false.

            // 2. Perform ! ReadableStreamDefaultControllerCallPullIfNeeded(controller).
        }
    }
    // 8. Upon rejection of pullPromise with reason e,
    {
        // 1. Perform ! ReadableStreamDefaultControllerError(controller, e).
    }
    todo!()
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-should-call-pull
// ReadableStreamDefaultControllerShouldCallPull(controller) performs the following steps:
pub fn readable_stream_default_controller_should_call_pull(
    controller: &ReadableStreamDefaultController,
) -> bool {
    // 1. Let stream be controller.[[stream]].

    // 2. If ! ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is false, return false.

    // 3. If controller.[[started]] is false, return false.

    // 4. If ! IsReadableStreamLocked(stream) is true and ! ReadableStreamGetNumReadRequests(stream) > 0, return true.

    // 5. Let desiredSize be ! ReadableStreamDefaultControllerGetDesiredSize(controller).

    // 6. Assert: desiredSize is not null.

    // 7. If desiredSize > 0, return true.

    // 8. Return false.

    todo!()
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-clear-algorithms
// ReadableStreamDefaultControllerClearAlgorithms(controller) is called once the stream is closed or errored and the algorithms will not be executed any more. By removing the algorithm references it permits the underlying source object to be garbage collected even if the ReadableStream itself is still referenced.
pub fn readable_stream_default_controller_clear_algorithms(
    controller: &mut ReadableStreamDefaultController,
) {
    // 1. Set controller.[[pullAlgorithm]] to undefined.

    // 2. Set controller.[[cancelAlgorithm]] to undefined.

    // 3. Set controller.[[strategySizeAlgorithm]] to undefined.
    todo!()
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-close
// ReadableStreamDefaultControllerClose(controller) performs the following steps:
pub fn readable_stream_default_controller_close(
    controller: &mut ReadableStreamDefaultController,
    context: &mut Context<'_>,
) {
    // 1. If ! ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is false, return.
    if readable_stream_default_controller_can_close_or_enqueue(controller) == false {
        return;
    }
    // 2. Let stream be controller.[[stream]].
    let mut stream_gc_cell = controller.stream.deref_mut(); // TODO inline?
    let mut stream = stream_gc_cell.deref_mut();
    // 3. Set controller.[[closeRequested]] to true.
    controller.close_requested = true;
    // 4. If controller.[[queue]] is empty,
    if controller.queue.is_empty() {
        // 1. Perform ! ReadableStreamDefaultControllerClearAlgorithms(controller).
        // readable_stream_default_controller_clear_algorithms(controller); // TODO https://tezos-dev.slack.com/archives/C061SSDBN69/p1700052360766569
        // 2. Perform ! ReadableStreamClose(stream).
        readable_stream_close(stream, context);
    }
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-enqueue
// ReadableStreamDefaultControllerEnqueue(controller, chunk) performs the following steps:
pub fn readable_stream_default_controller_enqueue(
    controller: &ReadableStreamDefaultController,
    chunk: idl::Chunk,
) {
    // 1. If ! ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is false, return.

    // 2. Let stream be controller.[[stream]].

    // 3. If ! IsReadableStreamLocked(stream) is true and ! ReadableStreamGetNumReadRequests(stream) > 0, perform ! ReadableStreamFulfillReadRequest(stream, chunk, false).

    // 4. Otherwise,
    {
        // 1. Let result be the result of performing controller.[[strategySizeAlgorithm]], passing in chunk, and interpreting the result as a completion record.

        // 2. If result is an abrupt completion,
        {
            // 1. Perform ! ReadableStreamDefaultControllerError(controller, result.[[Value]]).

            // 2. Return result.
        }
        // 3. Let chunkSize be result.[[Value]].

        // 4. Let enqueueResult be EnqueueValueWithSize(controller, chunk, chunkSize).

        // 5. If enqueueResult is an abrupt completion,
        {
            // 1. Perform ! ReadableStreamDefaultControllerError(controller, enqueueResult.[[Value]]).

            // 2. Return enqueueResult.
        }
    }
    // 5. Perform ! ReadableStreamDefaultControllerCallPullIfNeeded(controller).

    todo!()
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-error
// ReadableStreamDefaultControllerError(controller, e) performs the following steps:
pub fn readable_stream_default_controller_error(
    controller: &ReadableStreamDefaultController,
    e: JsError,
) {
    // 1. Let stream be controller.[[stream]].

    // 2. If stream.[[state]] is not "readable", return.

    // 3. Perform ! ResetQueue(controller).

    // 4. Perform ! ReadableStreamDefaultControllerClearAlgorithms(controller).

    // 5. Perform ! ReadableStreamError(stream, e).
    todo!()
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-get-desired-size
// ReadableStreamDefaultControllerGetDesiredSize(controller) performs the following steps:
pub fn readable_stream_default_controller_get_desired_size(
    controller: &ReadableStreamDefaultController,
) -> JsNullable<idl::Number> {
    // 1. Let state be controller.[[stream]].[[state]].
    let stream = controller.stream.deref(); // TODO inline?
    let state = &stream.state;
    // 2. If state is "errored", return null.
    // 3. If state is "closed", return 0.
    // 4. Return controller.[[strategyHWM]] − controller.[[queueTotalSize]].
    match state {
        ReadableStreamState::Errored => todo!(),
        ReadableStreamState::Closed => (0 as idl::Number).into(),
        ReadableStreamState::Readable => {
            (controller.strategy_hwm - controller.queue_total_size).into()
        }
    }
}

// https://streams.spec.whatwg.org/#rs-default-controller-has-backpressure
// ReadableStreamDefaultControllerHasBackpressure(controller) is used in the implementation of TransformStream. It performs the following steps:
pub fn readable_stream_default_controller_has_backpressure(
    controller: &ReadableStreamDefaultController,
) -> bool {
    // 1. If ! ReadableStreamDefaultControllerShouldCallPull(controller) is true, return false.
    // 2. Otherwise, return true.
    if readable_stream_default_controller_should_call_pull(controller) {
        return false;
    } else {
        return true;
    }
}

// https://streams.spec.whatwg.org/#readable-stream-default-controller-can-close-or-enqueue
// ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) performs the following steps:
pub fn readable_stream_default_controller_can_close_or_enqueue(
    controller: &ReadableStreamDefaultController,
) -> bool {
    // 1. Let state be controller.[[stream]].[[state]].

    // 2. If controller.[[closeRequested]] is false and state is "readable", return true.

    // 3. Otherwise, return false.
    todo!()
}
// Note: The case where controller.[[closeRequested]] is false, but state is not "readable", happens when the stream is errored via controller.error(), or when it is closed without its controller’s controller.close() method ever being called: e.g., if the stream was closed by a call to stream.cancel().

// https://streams.spec.whatwg.org/#set-up-readable-stream-default-controller
// SetUpReadableStreamDefaultController(stream, controller, startAlgorithm, pullAlgorithm, cancelAlgorithm, highWaterMark, sizeAlgorithm) performs the following steps:
pub fn set_up_readable_stream_default_controller(
    stream: &ReadableStream,
    controller: &ReadableStreamDefaultController,
    startAlgorithm: Todo,
    pullAlgorithm: Todo,
    cancelAlgorithm: Todo,
    highWaterMark: Todo,
    sizeAlgorithm: Todo,
) {
    // 1. Assert: stream.[[controller]] is undefined.

    // 2. Set controller.[[stream]] to stream.

    // 3. Perform ! ResetQueue(controller).

    // 4. Set controller.[[started]], controller.[[closeRequested]], controller.[[pullAgain]], and controller.[[pulling]] to false.

    // 5. Set controller.[[strategySizeAlgorithm]] to sizeAlgorithm and controller.[[strategyHWM]] to highWaterMark.

    // 6. Set controller.[[pullAlgorithm]] to pullAlgorithm.

    // 7. Set controller.[[cancelAlgorithm]] to cancelAlgorithm.

    // 8. Set stream.[[controller]] to controller.

    // 9. Let startResult be the result of performing startAlgorithm. (This might throw an exception.)

    // 10. Let startPromise be a promise resolved with startResult.

    // 11. Upon fulfillment of startPromise,
    {
        // 1. Set controller.[[started]] to true.

        // 2. Assert: controller.[[pulling]] is false.

        // 3. Assert: controller.[[pullAgain]] is false.

        // 4. Perform ! ReadableStreamDefaultControllerCallPullIfNeeded(controller).
    }
    // 12. Upon rejection of startPromise with reason r,
    {
        // 1. Perform ! ReadableStreamDefaultControllerError(controller, r).
    }
}

// https://streams.spec.whatwg.org/#set-up-readable-stream-default-controller-from-underlying-source
// SetUpReadableStreamDefaultControllerFromUnderlyingSource(stream, underlyingSource, underlyingSourceDict, highWaterMark, sizeAlgorithm) performs the following steps:
pub fn set_up_readable_stream_default_controller_from_underlying_source(
    stream: &ReadableStream,
    underlyingSource: &UnderlyingSource,
    underlyingSourceDict: Todo,
    highWaterMark: Todo,
    sizeAlgorithm: Todo,
) {
    // 1. Let controller be a new ReadableStreamDefaultController.

    // 2. Let startAlgorithm be an algorithm that returns undefined.

    // 3. Let pullAlgorithm be an algorithm that returns a promise resolved with undefined.

    // 4. Let cancelAlgorithm be an algorithm that returns a promise resolved with undefined.

    // 5. If underlyingSourceDict["start"] exists, then set startAlgorithm to an algorithm which returns the result of invoking underlyingSourceDict["start"] with argument list « controller » and callback this value underlyingSource.

    // 6. If underlyingSourceDict["pull"] exists, then set pullAlgorithm to an algorithm which returns the result of invoking underlyingSourceDict["pull"] with argument list « controller » and callback this value underlyingSource.

    // 7. If underlyingSourceDict["cancel"] exists, then set cancelAlgorithm to an algorithm which takes an argument reason and returns the result of invoking underlyingSourceDict["cancel"] with argument list « reason » and callback this value underlyingSource.

    // 8. Perform ? SetUpReadableStreamDefaultController(stream, controller, startAlgorithm, pullAlgorithm, cancelAlgorithm, highWaterMark, sizeAlgorithm).

    todo!()
}
