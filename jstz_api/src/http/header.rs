//! `jstz`'s implementation of JavaScript's `Headers` Web API Class.
//!
//! Represents response/request headers, allowing you to query them and take different
//! actions depending on the results in `jstz` contracts.
//!
//! FIXME: This is not spec compliant
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `Headers` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Headers
//! [spec]: https://fetch.spec.whatwg.org/#headers-class

use std::str::FromStr;

use boa_engine::{
    builtins::{self},
    object::{builtins::JsArray, Object},
    value::TryFromJs,
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use derive_more::{Deref, DerefMut};
use http::{header::Entry, HeaderMap, HeaderName, HeaderValue};
use jstz_core::{
    native::{register_global_class, ClassBuilder, JsNativeObject, NativeClass},
    value::IntoJs,
};
use jstz_crypto::public_key_hash::PublicKeyHash;
#[derive(Default, Clone, Deref, DerefMut)]
pub struct Headers {
    headers: HeaderMap,
}

impl Headers {
    pub fn from_http_headers(
        headers: http::HeaderMap,
        _context: &mut Context<'_>,
    ) -> JsResult<Self> {
        Ok(Self { headers })
    }

    pub fn to_http_headers(&self) -> http::HeaderMap {
        self.headers.clone()
    }
}

impl Finalize for Headers {}

unsafe impl Trace for Headers {
    empty_trace!();
}

impl From<HeaderMap> for Headers {
    fn from(headers: HeaderMap) -> Self {
        Self { headers }
    }
}

impl Headers {
    /// Creates a new Headers object
    pub fn new() -> Self {
        Self {
            headers: HeaderMap::new(),
        }
    }

    /// Appends a new value onto an existing header inside a Headers object, or adds the
    /// header if it does not already exist.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-append
    pub fn append(&mut self, name: &str, value: &str) -> JsResult<()> {
        self.headers
            .append(str_to_header_name(name)?, str_to_header_value(value)?);
        Ok(())
    }

    /// Deletes a header from a Headers object.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-delete
    pub fn remove(&mut self, name: &str) -> JsResult<()> {
        let name = str_to_header_name(name)?;
        match self.headers.entry(name) {
            Entry::Occupied(entry) => {
                entry.remove_entry_mult();
                Ok(())
            }
            Entry::Vacant(_) => Ok(()),
        }
    }

    /// Returns a String sequence of all the values of a header within a Headers object with a given name.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-get
    pub fn get(&self, name: &str) -> JsResult<Option<Header>> {
        let name = str_to_header_name(name)?;
        let mut values = self.headers.get_all(name).into_iter();
        match values.size_hint() {
            (0, _) => Ok(None),
            (1, Some(1)) => {
                let header = values.next().expect("Expect 1 header");

                Ok(Some(Header::Single(header_value_to_js_string(header)?)))
            }
            (1, None) => {
                let values = values
                    .map(header_value_to_js_string)
                    .collect::<JsResult<Vec<JsString>>>()?;

                Ok(Some(Header::Multiple(values)))
            }
            _ => todo!(),
        }
    }

    /// Returns a boolean stating whether a Headers object contains a certain header.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-has
    pub fn contains(&self, name: &str) -> JsResult<bool> {
        let name = str_to_header_name(name)?;
        Ok(self.headers.contains_key(&name))
    }

    /// Sets a new value for an existing header inside a Headers object, or adds the header if it does not already exist.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-set
    pub fn set(&mut self, name: &str, value: &str) -> JsResult<()> {
        let name = str_to_header_name(name)?;
        let value = str_to_header_value(value)?;
        self.headers.insert(name, value);
        Ok(())
    }
}

pub struct HeadersClass;

impl Headers {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Console`")
                    .into()
            })
    }
    pub fn set_referer(this: &JsValue, referer: &PublicKeyHash) -> JsResult<()> {
        let mut headers = Headers::try_from_js(this)?;
        if headers.contains("Referer")? {
            return Err(JsNativeError::eval()
                .with_message("Referer header should not be set")
                .into());
        }
        headers.set("Referer", &referer.to_base58())
    }
}

impl HeadersClass {
    fn append(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut headers = Headers::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: String = args.get_or_undefined(1).try_js_into(context)?;

        headers.append(&name, &value)?;

        Ok(JsValue::undefined())
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut headers = Headers::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;

        headers.remove(&name)?;

        Ok(JsValue::undefined())
    }

    fn get(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let headers = Headers::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;

        let header = headers.get(&name)?;

        Ok(match header {
            Some(header) => header.into_js(context),
            None => JsValue::null(),
        })
    }

    fn has(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let headers = Headers::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;

        Ok(headers.contains(&name)?.into())
    }

    fn set(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut headers = Headers::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: String = args.get_or_undefined(1).try_js_into(context)?;

        headers.set(&name, &value)?;

        Ok(JsValue::undefined())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HeaderEntry {
    name: String,
    value: String,
}

impl TryFromJs for HeaderEntry {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let arr: JsArray = value.try_js_into(context)?;

        let name: String = arr.get(0, context)?.try_js_into(context)?;
        let value: String = arr.get(1, context)?.try_js_into(context)?;

        Ok(Self { name, value })
    }
}

fn js_array_to_header_entries(
    obj: &JsObject,
    context: &mut Context<'_>,
) -> JsResult<Vec<HeaderEntry>> {
    let arr = JsArray::from_object(obj.clone())?;

    let mut vec = vec![];

    let length = arr.length(context)?;
    for i in 0..length {
        vec.push(HeaderEntry::try_from_js(&arr.get(i, context)?, context)?)
    }

    Ok(vec)
}

fn str_to_header_name(str: &str) -> JsResult<HeaderName> {
    HeaderName::try_from(&str.to_ascii_lowercase()).map_err(|_| {
        JsError::from_native(JsNativeError::typ().with_message("Invalid header name"))
    })
}

fn str_to_header_value(str: &str) -> JsResult<HeaderValue> {
    HeaderValue::try_from(str).map_err(|_| {
        JsError::from_native(JsNativeError::typ().with_message("Invalid header value"))
    })
}

fn header_value_to_js_string(header_value: &HeaderValue) -> JsResult<JsString> {
    let str = header_value.to_str().map_err(|_| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("Failed to convert `HeaderValue` to `&str`"),
        )
    })?;

    Ok(JsString::from_str(str).expect("Infallible"))
}

/// The `HeadersInit` enum
///
/// More information:
///  - [WHATWG specification][spec]
///
/// [spec] https://fetch.spec.whatwg.org/#typedefdef-headersinit
pub enum HeadersInit {
    New(Vec<HeaderEntry>),
    Existing(Headers),
}

impl Default for HeadersInit {
    fn default() -> Self {
        Self::Existing(Headers::default())
    }
}

impl Headers {
    pub fn from_init(init: HeadersInit) -> JsResult<Headers> {
        match init {
            HeadersInit::New(entries) => {
                let mut headers = Headers::default();
                for entry in entries {
                    headers.append(&entry.name, &entry.value)?
                }
                Ok(headers)
            }
            HeadersInit::Existing(headers) => Ok(headers),
        }
    }
}

impl TryFromJs for HeadersInit {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("Failed to convert js value into js object"),
            )
        })?;

        if obj.is_array() {
            Ok(Self::New(js_array_to_header_entries(obj, context)?))
        } else if obj.is_native_object() {
            let headers =
                obj.downcast_ref::<Headers>().ok_or_else(|| {
                    JsError::from_native(JsNativeError::typ().with_message(
                        "Failed to convert js object into Rust type `Headers`",
                    ))
                })?;

            Ok(Self::Existing(headers.clone()))
        } else {
            // TODO: Expose `enumerable_own_property_names` in Boa
            let arr = builtins::object::Object::entries(
                &JsValue::undefined(),
                &[value.clone()],
                context,
            )?
            .to_object(context)
            .expect("Expected array from `Object.entries`");

            Ok(Self::New(js_array_to_header_entries(&arr, context)?))
        }
    }
}

impl TryFromJs for Headers {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let init: HeadersInit = value.try_js_into(context)?;

        Headers::from_init(init)
    }
}

// FIXME: This representation isn't spec compliant.
// The spec defines that we should join multiple headers into a single USVString
// separated by `,` (with the exception of the `Set-Cookie`) header.
pub enum Header {
    Single(JsString),
    Multiple(Vec<JsString>),
}

impl IntoJs for Header {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        match self {
            Header::Single(header) => header.into(),
            Header::Multiple(headers) => {
                let headers: Vec<JsValue> =
                    headers.into_iter().map(|string| string.into()).collect();

                JsArray::from_iter(headers, context).into()
            }
        }
    }
}

impl NativeClass for HeadersClass {
    type Instance = Headers;

    const NAME: &'static str = "Headers";

    fn constructor(
        _this: &JsNativeObject<Headers>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Headers> {
        match args.get(0) {
            None => Ok(Headers::default()),
            Some(value) => value.try_js_into(context),
        }
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        class
            .method(
                "append",
                2,
                NativeFunction::from_fn_ptr(HeadersClass::append),
            )
            .method(
                "delete",
                1,
                NativeFunction::from_fn_ptr(HeadersClass::delete),
            )
            .method("get", 1, NativeFunction::from_fn_ptr(HeadersClass::get))
            .method("has", 1, NativeFunction::from_fn_ptr(HeadersClass::has))
            .method("set", 2, NativeFunction::from_fn_ptr(HeadersClass::set));

        Ok(())
    }
}

pub struct HeadersApi;

impl jstz_core::Api for HeadersApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<HeadersClass>(context)
            .expect("The `Headers` class shouldn't exist yet")
    }
}
