//! [Streams Standard - § 4.2.3. The underlying source API][https://streams.spec.whatwg.org/#underlying-source-api]

use std::str::FromStr;

use boa_engine::{
    object::builtins::JsPromise, value::TryFromJs, Context, JsData, JsNativeError,
    JsObject, JsResult, JsValue,
};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::{
    impl_into_js_from_into,
    js_fn::{JsCallable, JsFn},
    native::JsNativeObject,
    value::IntoJs,
};

use crate::{idl, stream::tmp::*};

/// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#underlying-source-api]
/// > ```notrust
/// > dictionary UnderlyingSource {
/// >   UnderlyingSourceStartCallback start;
/// >   UnderlyingSourcePullCallback pull;
/// >   UnderlyingSourceCancelCallback cancel;
/// >   ReadableStreamType type;
/// >   \[EnforceRange\] unsigned long long autoAllocateChunkSize;
/// > };
/// > ```
#[derive(Debug, JsData)]
pub struct UnderlyingSource {
    /// A reference to the [`JsObject`] from which the [`UnderlyingSource`] was build, used as `this` parameter when calling the methods of the [`UnderlyingSource`].
    ///
    /// [Streams Standard - § 4.2.4.][https://streams.spec.whatwg.org/#rs-prototype]
    /// > Note: We cannot declare the underlyingSource argument as having the UnderlyingSource type directly, because doing so would lose the reference to the original object. We need to retain the object so we can invoke the various methods on it.
    pub this: JsObject,

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-start]
    /// > **`start(controller)`, of type UnderlyingSourceStartCallback**
    /// >
    /// > A function that is called immediately during creation of the ReadableStream.
    /// >
    /// >  Typically this is used to adapt a push source by setting up relevant event listeners, as in the example of § 10.1 A readable stream with an underlying push source (no backpressure support), or to acquire access to a pull source, as in § 10.4 A readable stream with an underlying pull source.
    /// >
    /// >  If this setup process is asynchronous, it can return a promise to signal success or failure; a rejected promise will error the stream. Any thrown exceptions will be re-thrown by the ReadableStream() constructor.
    pub start: Option<UnderlyingSourceStartCallback>,

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-pull]
    /// > **`pull(controller)`, of type UnderlyingSourcePullCallback**
    /// >
    /// > A function that is called whenever the stream’s internal queue of chunks becomes not full, i.e. whenever the queue’s desired size becomes positive. Generally, it will be called repeatedly until the queue reaches its high water mark (i.e. until the desired size becomes non-positive).
    /// >
    /// > For push sources, this can be used to resume a paused flow, as in § 10.2 A readable stream with an underlying push source and backpressure support. For pull sources, it is used to acquire new chunks to enqueue into the stream, as in § 10.4 A readable stream with an underlying pull source.
    /// >
    /// > This function will not be called until start() successfully completes. Additionally, it will only be called repeatedly if it enqueues at least one chunk or fulfills a BYOB request; a no-op pull() implementation will not be continually called.
    /// >
    /// > If the function returns a promise, then it will not be called again until that promise fulfills. (If the promise rejects, the stream will become errored.) This is mainly used in the case of pull sources, where the promise returned represents the process of acquiring a new chunk. Throwing an exception is treated the same as returning a rejected promise.
    pub pull: Option<UnderlyingSourcePullCallback>,

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-cancel]
    /// > **`cancel(reason)`, of type UnderlyingSourceCancelCallback**
    ///
    /// > A function that is called whenever the consumer cancels the stream, via stream.cancel() or reader.cancel(). It takes as its argument the same value as was passed to those methods by the consumer.
    /// >
    /// > Readable streams can additionally be canceled under certain conditions during piping; see the definition of the pipeTo() method for more details.
    /// >
    /// > For all streams, this is generally used to release access to the underlying resource; see for example § 10.1 A readable stream with an underlying push source (no backpressure support).
    /// >
    /// > If the shutdown process is asynchronous, it can return a promise to signal success or failure; the result will be communicated via the return value of the cancel() method that was called. Throwing an exception is treated the same as returning a rejected promise.
    /// >
    /// > *Even if the cancelation process fails, the stream will still close; it will not be put into an errored state. This is because a failure in the cancelation process doesn’t matter to the consumer’s view of the stream, once they’ve expressed disinterest in it by canceling. The failure is only communicated to the immediate caller of the corresponding method.*
    /// >
    /// > *This is different from the behavior of the close and abort options of a WritableStream's underlying sink, which upon failure put the corresponding WritableStream into an errored state. Those correspond to specific actions the producer is requesting and, if those actions fail, they indicate something more persistently wrong.*
    pub cancel: Option<UnderlyingSourceCancelCallback>,

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-type]
    /// > **`type` (byte streams only), of type ReadableStreamType**
    /// >
    /// > Can be set to "bytes" to signal that the constructed ReadableStream is a readable byte stream. This ensures that the resulting ReadableStream will successfully be able to vend BYOB readers via its getReader() method. It also affects the controller argument passed to the start() and pull() methods; see below.
    /// >
    /// > For an example of how to set up a readable byte stream, including using the different controller interface, see § 10.3 A readable byte stream with an underlying push source (no backpressure support).
    /// >
    /// > Setting any value other than "bytes" or undefined will cause the ReadableStream() constructor to throw an exception.
    pub r#type: Option<ReadableStreamType>,

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-autoallocatechunksize]
    /// > **`autoAllocateChunkSize` (byte streams only), of type unsigned long long**
    /// >
    /// > Can be set to a positive integer to cause the implementation to automatically allocate buffers for the underlying source code to write into. In this case, when a consumer is using a default reader, the stream implementation will automatically allocate an ArrayBuffer of the given size, so that controller.byobRequest is always present, as if the consumer was using a BYOB reader.
    /// >
    /// > This is generally used to cut down on the amount of code needed to handle consumers that use default readers, as can be seen by comparing § 10.3 A readable byte stream with an underlying push source (no backpressure support) without auto-allocation to § 10.5 A readable byte stream with an underlying pull source with auto-allocation.
    pub auto_allocate_chunk_size: Option<idl::UnsignedLongLong>, // TODO [EnforceRange]
}

impl Finalize for UnderlyingSource {
    fn finalize(&self) {}
}

unsafe impl Trace for UnderlyingSource {
    custom_trace!(this, mark, {
        mark(&this.this);
        mark(&this.start);
        mark(&this.pull);
        mark(&this.cancel);
    });
}

// TODO derive this implementation with a macro?
impl TryFromJs for UnderlyingSource {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let this = value.to_object(context)?;
        let start: Option<UnderlyingSourceStartCallback> =
            get_jsobject_property(&this, "start", context)?.try_js_into(context)?;
        let pull: Option<UnderlyingSourcePullCallback> =
            get_jsobject_property(&this, "pull", context)?.try_js_into(context)?;
        let cancel: Option<UnderlyingSourceCancelCallback> =
            get_jsobject_property(&this, "cancel", context)?.try_js_into(context)?;
        let r#type =
            get_jsobject_property(&this, "type", context)?.try_js_into(context)?;
        let auto_allocate_chunk_size =
            get_jsobject_property(&this, "autoAllocateChunkSize", context)?
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

impl From<UnderlyingSource> for JsValue {
    fn from(value: UnderlyingSource) -> JsValue {
        value.this.into()
    }
}

impl_into_js_from_into!(UnderlyingSource);

/// This trait makes calling the functions stored in the fields `start`, `pull`, and `cancel` of an [`UnderlyingSource`] easier, using the defaults from [`SetUpReadableStreamDefaultControllerFromUnderlyingSource`][spec1] / [`SetUpReadableByteStreamControllerFromUnderlyingSource`][spec2] when they are missing.
///
/// [spec1]: https://streams.spec.whatwg.org/#set-up-readable-stream-default-controller-from-underlying-source
/// [spec2]: https://streams.spec.whatwg.org/#set-up-readable-byte-stream-controller-from-underlying-source
pub trait UnderlyingSourceTrait {
    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-start]
    /// > **`start(controller)`, of type UnderlyingSourceStartCallback**
    /// >
    /// > A function that is called immediately during creation of the ReadableStream.
    /// >
    /// >  Typically this is used to adapt a push source by setting up relevant event listeners, as in the example of § 10.1 A readable stream with an underlying push source (no backpressure support), or to acquire access to a pull source, as in § 10.4 A readable stream with an underlying pull source.
    /// >
    /// >  If this setup process is asynchronous, it can return a promise to signal success or failure; a rejected promise will error the stream. Any thrown exceptions will be re-thrown by the ReadableStream() constructor.
    fn start(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<JsValue>;

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-pull]
    /// > **`pull(controller)`, of type UnderlyingSourcePullCallback**
    /// >
    /// > A function that is called whenever the stream’s internal queue of chunks becomes not full, i.e. whenever the queue’s desired size becomes positive. Generally, it will be called repeatedly until the queue reaches its high water mark (i.e. until the desired size becomes non-positive).
    /// >
    /// > For push sources, this can be used to resume a paused flow, as in § 10.2 A readable stream with an underlying push source and backpressure support. For pull sources, it is used to acquire new chunks to enqueue into the stream, as in § 10.4 A readable stream with an underlying pull source.
    /// >
    /// > This function will not be called until start() successfully completes. Additionally, it will only be called repeatedly if it enqueues at least one chunk or fulfills a BYOB request; a no-op pull() implementation will not be continually called.
    /// >
    /// > If the function returns a promise, then it will not be called again until that promise fulfills. (If the promise rejects, the stream will become errored.) This is mainly used in the case of pull sources, where the promise returned represents the process of acquiring a new chunk. Throwing an exception is treated the same as returning a rejected promise.
    fn pull(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>>;

    /// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#dom-underlyingsource-cancel]
    /// > **`cancel(reason)`, of type UnderlyingSourceCancelCallback**
    ///
    /// > A function that is called whenever the consumer cancels the stream, via stream.cancel() or reader.cancel(). It takes as its argument the same value as was passed to those methods by the consumer.
    /// >
    /// > Readable streams can additionally be canceled under certain conditions during piping; see the definition of the pipeTo() method for more details.
    /// >
    /// > For all streams, this is generally used to release access to the underlying resource; see for example § 10.1 A readable stream with an underlying push source (no backpressure support).
    /// >
    /// > If the shutdown process is asynchronous, it can return a promise to signal success or failure; the result will be communicated via the return value of the cancel() method that was called. Throwing an exception is treated the same as returning a rejected promise.
    /// >
    /// > *Even if the cancelation process fails, the stream will still close; it will not be put into an errored state. This is because a failure in the cancelation process doesn’t matter to the consumer’s view of the stream, once they’ve expressed disinterest in it by canceling. The failure is only communicated to the immediate caller of the corresponding method.*
    /// >
    /// > *This is different from the behavior of the close and abort options of a WritableStream's underlying sink, which upon failure put the corresponding WritableStream into an errored state. Those correspond to specific actions the producer is requesting and, if those actions fail, they indicate something more persistently wrong.*
    fn cancel(
        &self,
        reason: Option<JsValue>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>>;
}

/// [`UndefinedUnderlyingSource`] is a trivial struct meant to hold the default implementations of the methods of [UnderlyingSourceTrait] taken from steps 2., 3., and 4. of [`SetUpReadableStreamDefaultControllerFromUnderlyingSource`][https://streams.spec.whatwg.org/#set-up-readable-stream-default-controller-from-underlying-source] / [`SetUpReadableByteStreamControllerFromUnderlyingSource`][https://streams.spec.whatwg.org/#set-up-readable-byte-stream-controller-from-underlying-source].
#[derive(Default)]
pub struct UndefinedUnderlyingSource {}

impl UnderlyingSourceTrait for UndefinedUnderlyingSource {
    fn start(
        &self,
        _controller: JsNativeObject<ReadableStreamController>,
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        Ok(JsValue::Undefined)
    }

    fn pull(
        &self,
        _controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        Ok(Some(JsPromise::resolve(JsValue::Undefined, context)))
    }

    fn cancel(
        &self,
        _reason: Option<JsValue>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        Ok(Some(JsPromise::resolve(JsValue::Undefined, context)))
    }
}

impl UnderlyingSourceTrait for UnderlyingSource {
    fn start(
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
            UndefinedUnderlyingSource::default().start(controller, context)
        }
    }

    fn pull(
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
            UndefinedUnderlyingSource::default().pull(controller, context)
        }
    }

    fn cancel(
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
            UndefinedUnderlyingSource::default().cancel(reason, context)
        }
    }
}

impl UnderlyingSourceTrait for Option<UnderlyingSource> {
    fn start(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        match self {
            Some(underlying_source) => underlying_source.start(controller, context),
            None => UndefinedUnderlyingSource::default().start(controller, context),
        }
    }

    fn pull(
        &self,
        controller: JsNativeObject<ReadableStreamController>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        match self {
            Some(underlying_source) => underlying_source.pull(controller, context),
            None => UndefinedUnderlyingSource::default().pull(controller, context),
        }
    }

    fn cancel(
        &self,
        reason: Option<JsValue>,
        context: &mut Context,
    ) -> JsResult<Option<JsPromise>> {
        match self {
            Some(underlying_source) => underlying_source.cancel(reason, context),
            None => UndefinedUnderlyingSource::default().cancel(reason, context),
        }
    }
}

/// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#typedefdef-readablestreamcontroller]
/// > `typedef (ReadableStreamDefaultController or ReadableByteStreamController) ReadableStreamController;`
#[derive(Debug, JsData)]
pub enum ReadableStreamController {
    DefaultController(ReadableStreamDefaultController),
    ByteController(ReadableByteStreamController),
}

impl Finalize for ReadableStreamController {
    fn finalize(&self) {}
}

unsafe impl Trace for ReadableStreamController {
    custom_trace!(this, mark, {
        match this {
            ReadableStreamController::DefaultController(value) => mark(value),
            ReadableStreamController::ByteController(value) => mark(value),
        }
    });
}

/// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#callbackdef-underlyingsourcestartcallback]
/// > `callback UnderlyingSourceStartCallback = any (ReadableStreamController controller);`
pub type UnderlyingSourceStartCallback =
    JsFn<JsObject, (JsNativeObject<ReadableStreamController>,), idl::Any>;

/// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#callbackdef-underlyingsourcepullcallback]
/// > `callback UnderlyingSourcePullCallback = Promise<undefined> (ReadableStreamController controller);`
pub type UnderlyingSourcePullCallback =
    JsFn<JsObject, (JsNativeObject<ReadableStreamController>,), Option<JsPromise>>;

/// [Streams Standard - § 4.2.3.][https://streams.spec.whatwg.org/#callbackdef-underlyingsourcecancelcallback]
/// > `callback UnderlyingSourceCancelCallback = Promise<undefined> (optional any reason);`
pub type UnderlyingSourceCancelCallback = JsFn<JsObject, (idl::Any,), Option<JsPromise>>;

/// [ReadableStreamType] represents the singleton type `{"bytes"}`.
#[derive(Debug, PartialEq)]
pub enum ReadableStreamType {
    Bytes,
}

impl From<ReadableStreamType> for &str {
    fn from(value: ReadableStreamType) -> &'static str {
        match value {
            ReadableStreamType::Bytes => "bytes",
        }
    }
}

impl FromStr for ReadableStreamType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bytes" => Ok(ReadableStreamType::Bytes),
            _ => Err(()),
        }
    }
}

impl IntoJs for ReadableStreamType {
    fn into_js(self, context: &mut Context) -> JsValue {
        let str: &str = self.into();
        String::from(str).into_js(context)
    }
}

impl TryFromJs for ReadableStreamType {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let str = String::try_from_js(value, context)?;
        ReadableStreamType::from_str(&str).map_err(|()| {
            JsNativeError::typ()
                .with_message(format!(
                    "{} is not a valid value for enumeration ReadableStreamType.",
                    str
                ))
                .into()
        })
    }
}
