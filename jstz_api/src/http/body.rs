//! `jstz`'s implementation of JavaScript's `Body` Web API mixin.
//!
//! Represents response/request body.
//!
//! FIXME: This implementation only implements a subset of the spec.
//! The following is missing:
//!  - Support for streams
//!  - Support for blobs
//!  - Support for form data
//!
//! More information:
//!  - [WHATWG `Headers` specification][spec]
//!
//! [spec]: https://fetch.spec.whatwg.org/#body-mixin

use boa_engine::{
    object::builtins::{JsArrayBuffer, JsPromise},
    value::TryFromJs,
    Context, JsError, JsNativeError, JsResult, JsString, JsValue,
};
use boa_gc::{Finalize, Trace};

#[derive(Trace, Finalize, Clone)]
enum Inner {
    Text(JsString),
    Bytes(Vec<u8>),
}

fn bytes_to_string(bytes: &Vec<u8>) -> JsResult<String> {
    String::from_utf8(bytes.clone()).map_err(|_| {
        JsError::from_native(
            JsNativeError::typ().with_message("Failed to convert bytes into utf8 text"),
        )
    })
}

impl Inner {
    fn text(&self) -> JsResult<JsString> {
        match self {
            Inner::Text(string) => Ok(string.clone()),
            Inner::Bytes(bytes) => {
                let string = bytes_to_string(bytes)?;
                Ok(JsString::from(string.as_str()))
            }
        }
    }

    fn string(&self) -> JsResult<String> {
        match self {
            Inner::Text(string) => Ok(string.to_std_string_escaped()),
            Inner::Bytes(bytes) => bytes_to_string(bytes),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Inner::Text(string) => string.to_std_string_escaped().into_bytes(),
            Inner::Bytes(bytes) => bytes.clone(),
        }
    }

    fn into_array_buffer(self, context: &mut Context<'_>) -> JsResult<JsArrayBuffer> {
        JsArrayBuffer::from_byte_block(self.bytes(), context)
    }
}

#[derive(Trace, Finalize, Clone)]
pub struct Body {
    inner: Option<Inner>,
}

impl Body {
    fn new(inner: Inner) -> Self {
        Self { inner: Some(inner) }
    }

    fn inner(&mut self) -> JsResult<Inner> {
        // Consumes the body
        match self.inner.take() {
            Some(inner) => Ok(inner),
            None => Err(JsError::from_native(
                JsNativeError::typ().with_message("Body is null or has been used"),
            )),
        }
    }

    /// Returns a `null` body
    pub fn null() -> Self {
        Self { inner: None }
    }

    /// Returns whether the body has been read from.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-body-bodyused
    pub fn is_used(&self) -> bool {
        // 1. Return true if this’s `body` is non-null and this’s
        //    body’s stream is disturbed; otherwise false.
        // FIXME: Support streams
        self.inner.is_none()
    }

    pub fn is_null(&self) -> bool {
        self.is_used()
    }

    /// Returns a promise fulfilled with body's content as an ArrayBuffer
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-body-arraybuffer
    pub fn array_buffer(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        let inner = self.inner()?;
        JsPromise::resolve(inner.into_array_buffer(context)?, context)
    }

    /// Returns a promise fulfilled with body's content as a string
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-body-text
    pub fn text(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        let inner = self.inner()?;
        JsPromise::resolve(inner.text()?, context)
    }

    /// Returns a promise fulfilled with body's content parsed as JSON
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-body-json
    pub fn json(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        let inner = self.inner()?;
        let json: serde_json::Value =
            serde_json::from_str(&inner.string()?).map_err(|_| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to convert `Body` to `serde_json::Value`"),
                )
            })?;

        JsPromise::resolve(JsValue::from_json(&json, context)?, context)
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::null()
    }
}

/// The `BodyInit` union.
///
/// More information:
///  - [WHATWG specification][spec]
///
/// [spec] https://fetch.spec.whatwg.org/#bodyinit-unions
pub enum BodyInit {
    Text(JsString),
    BufferSource(JsArrayBuffer),
}

impl TryFromJs for BodyInit {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if let Some(string) = value.as_string() {
            return Ok(Self::Text(string.clone()));
        };

        Ok(Self::BufferSource(JsArrayBuffer::try_from_js(
            value, context,
        )?))
    }
}

/// A body with type is a tuple that consists of a body (a body) and a
/// type (a header value or null).
///
/// More information:
///  - [WHATWG specification][spec]
///
/// [spec] https://fetch.spec.whatwg.org/#body-with-type
#[derive(Default)]
pub struct BodyWithType {
    pub body: Body,
    pub content_type: Option<&'static str>,
}

impl BodyWithType {
    /// Constructs a `BodyWithType` from the serialization of `value`.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-jsonF
    pub fn json(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let json = value.to_json(context)?;
        let bytes =
            JsArrayBuffer::from_byte_block(json.to_string().into_bytes(), context)?;
        let body = BodyWithType::from_init(BodyInit::BufferSource(bytes))?.body;
        Ok(Self {
            body,
            content_type: Some("application/json"),
        })
    }

    /// Extracts a body with type from a `BodyInit`.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#concept-bodyinit-extract
    pub fn from_init(init: BodyInit) -> JsResult<Self> {
        match init {
            BodyInit::Text(string) => {
                let body = Body::new(Inner::Text(string));

                Ok(Self {
                    body,
                    content_type: Some("text/plain;charset=UTF-8"),
                })
            }
            BodyInit::BufferSource(array_buffer) => {
                let bytes = array_buffer.take()?;

                let body = Body::new(Inner::Bytes(bytes));
                Ok(Self {
                    body,
                    content_type: None,
                })
            }
        }
    }
}

impl TryFromJs for BodyWithType {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let init: BodyInit = value.try_js_into(context)?;

        BodyWithType::from_init(init)
    }
}
