//! https://streams.spec.whatwg.org/#underlying-source-api

use boa_engine::{
    js_string, object::builtins::JsPromise, property::PropertyKey, value::TryFromJs,
    Context, JsNativeError, JsObject, JsResult, JsValue,
};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::{
    impl_into_js_from_into, js_fn::JsFn, native::JsNativeObject, value::IntoJs,
};
use std::str::FromStr;
use strum_macros::EnumString;

use crate::{idl, stream::tmp::*};

/// dictionary [UnderlyingSource][spec] {
///   UnderlyingSourceStartCallback start;
///   UnderlyingSourcePullCallback pull;
///   UnderlyingSourceCancelCallback cancel;
///   ReadableStreamType type;
///   \[EnforceRange\] unsigned long long autoAllocateChunkSize;
/// };
///
/// [Note][spec2]: We cannot declare the underlyingSource argument as having the UnderlyingSource type directly, because doing so would lose the reference to the original object. We need to retain the object so we can invoke the various methods on it.
///
/// [spec]: https://streams.spec.whatwg.org/#dictdef-underlyingsource
/// [spec2]: https://streams.spec.whatwg.org/#rs-constructor
#[derive(Debug)]
pub struct UnderlyingSource {
    /// TODO
    pub this: JsObject,

    /// **[start][spec](controller), of type UnderlyingSourceStartCallback**
    ///
    ///  A function that is called immediately during creation of the ReadableStream.
    ///
    ///  Typically this is used to adapt a push source by setting up relevant event listeners, as in the example of § 10.1 A readable stream with an underlying push source (no backpressure support), or to acquire access to a pull source, as in § 10.4 A readable stream with an underlying pull source.
    ///
    ///  If this setup process is asynchronous, it can return a promise to signal success or failure; a rejected promise will error the stream. Any thrown exceptions will be re-thrown by the ReadableStream() constructor.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-start
    pub start: Option<UnderlyingSourceStartCallback>,

    /// **[pull][spec](controller), of type UnderlyingSourcePullCallback**
    ///
    /// A function that is called whenever the stream’s internal queue of chunks becomes not full, i.e. whenever the queue’s desired size becomes positive. Generally, it will be called repeatedly until the queue reaches its high water mark (i.e. until the desired size becomes non-positive).
    ///
    /// For push sources, this can be used to resume a paused flow, as in § 10.2 A readable stream with an underlying push source and backpressure support. For pull sources, it is used to acquire new chunks to enqueue into the stream, as in § 10.4 A readable stream with an underlying pull source.
    ///
    /// This function will not be called until start() successfully completes. Additionally, it will only be called repeatedly if it enqueues at least one chunk or fulfills a BYOB request; a no-op pull() implementation will not be continually called.
    ///
    /// If the function returns a promise, then it will not be called again until that promise fulfills. (If the promise rejects, the stream will become errored.) This is mainly used in the case of pull sources, where the promise returned represents the process of acquiring a new chunk. Throwing an exception is treated the same as returning a rejected promise.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-pull
    pub pull: Option<UnderlyingSourcePullCallback>,

    /// **cancel(reason), of type UnderlyingSourceCancelCallback**
    /// A function that is called whenever the consumer cancels the stream, via stream.cancel() or reader.cancel(). It takes as its argument the same value as was passed to those methods by the consumer.
    ///
    /// Readable streams can additionally be canceled under certain conditions during piping; see the definition of the pipeTo() method for more details.
    ///
    // For all streams, this is generally used to release access to the underlying resource; see for example § 10.1 A readable stream with an underlying push source (no backpressure support).
    ///
    /// If the shutdown process is asynchronous, it can return a promise to signal success or failure; the result will be communicated via the return value of the cancel() method that was called. Throwing an exception is treated the same as returning a rejected promise.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-cancel
    ///
    /// *Even if the cancelation process fails, the stream will still close; it will not be put into an errored state. This is because a failure in the cancelation process doesn’t matter to the consumer’s view of the stream, once they’ve expressed disinterest in it by canceling. The failure is only communicated to the immediate caller of the corresponding method.*
    ///
    /// *This is different from the behavior of the close and abort options of a WritableStream's underlying sink, which upon failure put the corresponding WritableStream into an errored state. Those correspond to specific actions the producer is requesting and, if those actions fail, they indicate something more persistently wrong.*
    pub cancel: Option<UnderlyingSourceCancelCallback>,

    /// **[type][spec] (byte streams only), of type ReadableStreamType**
    ///
    /// Can be set to "bytes" to signal that the constructed ReadableStream is a readable byte stream. This ensures that the resulting ReadableStream will successfully be able to vend BYOB readers via its getReader() method. It also affects the controller argument passed to the start() and pull() methods; see below.
    ///
    /// For an example of how to set up a readable byte stream, including using the different controller interface, see § 10.3 A readable byte stream with an underlying push source (no backpressure support).
    ///
    /// Setting any value other than "bytes" or undefined will cause the ReadableStream() constructor to throw an exception.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-type
    pub r#type: Option<ReadableStreamType>,

    /// **[autoAllocateChunkSize][spec] (byte streams only), of type unsigned long long**
    ///
    /// Can be set to a positive integer to cause the implementation to automatically allocate buffers for the underlying source code to write into. In this case, when a consumer is using a default reader, the stream implementation will automatically allocate an ArrayBuffer of the given size, so that controller.byobRequest is always present, as if the consumer was using a BYOB reader.
    ///
    /// This is generally used to cut down on the amount of code needed to handle consumers that use default readers, as can be seen by comparing § 10.3 A readable byte stream with an underlying push source (no backpressure support) without auto-allocation to § 10.5 A readable byte stream with an underlying pull source with auto-allocation.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-autoallocatechunksize
    pub auto_allocate_chunk_size: Option<idl::UnsignedLongLong>, // TODO  [EnforceRange]
}

impl Finalize for UnderlyingSource {
    fn finalize(&self) {}
}

unsafe impl Trace for UnderlyingSource {
    custom_trace!(this, {
        mark(&this.this);
        mark(&this.start);
        mark(&this.pull);
        mark(&this.cancel);
    });
}

// TODO derive this implementation with a macro?
impl TryFromJs for UnderlyingSource {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        // TODO check that this function works as intended in all cases,
        // and move it either to a new derive macro for TryFromJs, or to JsObject
        #[allow(non_snake_case)]
        pub fn get_JsObject_property(
            obj: &JsObject,
            name: &str,
            context: &mut Context<'_>,
        ) -> JsResult<JsValue> {
            let key = PropertyKey::from(js_string!(name));
            let key2 = key.clone();
            let has_prop = obj.has_property(key, context)?;
            if has_prop {
                obj.get(key2, context)
            } else {
                Ok(JsValue::Undefined)
            }
        }

        let this = value.to_object(context)?;
        let start: Option<UnderlyingSourceStartCallback> =
            get_JsObject_property(&this, "start", context)?.try_js_into(context)?;
        let pull: Option<UnderlyingSourcePullCallback> =
            get_JsObject_property(&this, "pull", context)?.try_js_into(context)?;
        let cancel: Option<UnderlyingSourceCancelCallback> =
            get_JsObject_property(&this, "cancel", context)?.try_js_into(context)?;
        let r#type =
            get_JsObject_property(&this, "type", context)?.try_js_into(context)?;
        let auto_allocate_chunk_size =
            get_JsObject_property(&this, "autoAllocateChunkSize", context)?
                .try_js_into(context)?;
        Ok(UnderlyingSource {
            this,
            start,
            pull,
            cancel,
            r#type,
            auto_allocate_chunk_size,
        })
    }
}

impl Into<JsValue> for UnderlyingSource {
    fn into(self) -> JsValue {
        self.this.into()
    }
}

impl_into_js_from_into!(UnderlyingSource);

/// typedef (ReadableStreamDefaultController or ReadableByteStreamController) [ReadableStreamController][spec];
///
/// [spec]: https://streams.spec.whatwg.org/#typedefdef-readablestreamcontroller
#[derive(Debug)]
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
    custom_trace!(this, {
        match this {
            ReadableStreamController::DefaultController(value) => mark(value),
            ReadableStreamController::ByteController(value) => mark(value),
        }
    });
}

/// callback [UnderlyingSourceStartCallback][spec] = any (ReadableStreamController controller);
///
/// [spec]: https://streams.spec.whatwg.org/#callbackdef-underlyingsourcestartcallback
pub type UnderlyingSourceStartCallback =
    JsFn<JsObject, 1, (JsNativeObject<ReadableStreamController>,), idl::Any>;

/// callback [UnderlyingSourcePullCallback][spec] = Promise<undefined> (ReadableStreamController controller);
///
/// [spec]: https://streams.spec.whatwg.org/#callbackdef-underlyingsourcepullcallback
pub type UnderlyingSourcePullCallback =
    JsFn<JsObject, 1, (JsNativeObject<ReadableStreamController>,), Option<JsPromise>>;

/// callback [UnderlyingSourceCancelCallback][spec] = Promise<undefined> (optional any reason);
///
/// [spec]: https://streams.spec.whatwg.org/#callbackdef-underlyingsourcecancelcallback
pub type UnderlyingSourceCancelCallback =
    JsFn<JsObject, 1, (idl::Any,), Option<JsPromise>>;

impl UnderlyingSource {
    /// **[start][spec](controller), of type UnderlyingSourceStartCallback**
    ///
    ///  A function that is called immediately during creation of the ReadableStream.
    ///
    ///  Typically this is used to adapt a push source by setting up relevant event listeners, as in the example of § 10.1 A readable stream with an underlying push source (no backpressure support), or to acquire access to a pull source, as in § 10.4 A readable stream with an underlying pull source.
    ///
    ///  If this setup process is asynchronous, it can return a promise to signal success or failure; a rejected promise will error the stream. Any thrown exceptions will be re-thrown by the ReadableStream() constructor.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-start
    pub fn start(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        if let Some(ref start) = self.start {
            start.call(
                self.this.clone(), // TODO remove clone? https://tezos-dev.slack.com/archives/C061SSDBN69/p1701192316869399
                (controller,),
                context,
            )
        } else {
            Ok(JsValue::Undefined) // TODO doc
        }
    }

    /// **[pull][spec](controller), of type UnderlyingSourcePullCallback**
    ///
    /// A function that is called whenever the stream’s internal queue of chunks becomes not full, i.e. whenever the queue’s desired size becomes positive. Generally, it will be called repeatedly until the queue reaches its high water mark (i.e. until the desired size becomes non-positive).
    ///
    /// For push sources, this can be used to resume a paused flow, as in § 10.2 A readable stream with an underlying push source and backpressure support. For pull sources, it is used to acquire new chunks to enqueue into the stream, as in § 10.4 A readable stream with an underlying pull source.
    ///
    /// This function will not be called until start() successfully completes. Additionally, it will only be called repeatedly if it enqueues at least one chunk or fulfills a BYOB request; a no-op pull() implementation will not be continually called.
    ///
    /// If the function returns a promise, then it will not be called again until that promise fulfills. (If the promise rejects, the stream will become errored.) This is mainly used in the case of pull sources, where the promise returned represents the process of acquiring a new chunk. Throwing an exception is treated the same as returning a rejected promise.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-pull
    pub fn pull(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        if let Some(ref pull) = self.pull {
            pull.call(
                self.this.clone(), // TODO remove clone? https://tezos-dev.slack.com/archives/C061SSDBN69/p1701192316869399
                (controller,),
                context,
            )
        } else {
            JsPromise::resolve(JsValue::Undefined, context).map(Option::Some)
        }
    }

    /// **cancel(reason), of type UnderlyingSourceCancelCallback**
    /// A function that is called whenever the consumer cancels the stream, via stream.cancel() or reader.cancel(). It takes as its argument the same value as was passed to those methods by the consumer.
    ///
    /// Readable streams can additionally be canceled under certain conditions during piping; see the definition of the pipeTo() method for more details.
    ///
    // For all streams, this is generally used to release access to the underlying resource; see for example § 10.1 A readable stream with an underlying push source (no backpressure support).
    ///
    /// If the shutdown process is asynchronous, it can return a promise to signal success or failure; the result will be communicated via the return value of the cancel() method that was called. Throwing an exception is treated the same as returning a rejected promise.
    ///
    /// [spec]: https://streams.spec.whatwg.org/#dom-underlyingsource-cancel
    ///
    /// *Even if the cancelation process fails, the stream will still close; it will not be put into an errored state. This is because a failure in the cancelation process doesn’t matter to the consumer’s view of the stream, once they’ve expressed disinterest in it by canceling. The failure is only communicated to the immediate caller of the corresponding method.*
    ///
    /// *This is different from the behavior of the close and abort options of a WritableStream's underlying sink, which upon failure put the corresponding WritableStream into an errored state. Those correspond to specific actions the producer is requesting and, if those actions fail, they indicate something more persistently wrong.*
    pub fn cancel(
        &self,
        reason: Option<JsValue>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        if let Some(ref cancel) = self.cancel {
            cancel.call(
                self.this.clone(), // TODO remove clone? https://tezos-dev.slack.com/archives/C061SSDBN69/p1701192316869399
                (reason.unwrap_or(JsValue::Undefined),),
                context,
            )
        } else {
            JsPromise::resolve(JsValue::Undefined, context).map(Option::Some)
        }
    }
}

#[derive(Debug, EnumString, PartialEq)]
pub enum ReadableStreamType {
    #[strum(serialize = "bytes")]
    Bytes,
}

impl TryFromJs for ReadableStreamType {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let str = String::try_from_js(value, context)?;
        ReadableStreamType::from_str(&str).map_err(
            |strum::ParseError::VariantNotFound| {
                JsNativeError::typ().with_message("TODO").into()
            },
        )
    }
}
