//! `jstz`'s implementation of JavaScript's `Headers` Web API Class.
//!
//! Represents response/request headers, allowing you to query them and take different
//! actions depending on the results in `jstz` smart functions
//!
//! FIXME: This is not spec compliant
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `Headers` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Headers
//! [spec]: https://fetch.spec.whatwg.org/#headers-class

use std::{cell::RefCell, collections::BTreeMap, ops::DerefMut};

use boa_engine::{
    builtins, js_string,
    object::{builtins::JsArray, Object},
    value::TryFromJs,
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsSymbol,
    JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use derive_more::Deref;
use http::{header::Entry, HeaderMap, HeaderName, HeaderValue};
use jstz_core::{
    iterators::{PairIterable, PairIterableMethods, PairIteratorClass, PairValue},
    native::{register_global_class, ClassBuilder, JsNativeObject, NativeClass},
    value::IntoJs,
};
#[derive(Default, Clone, Deref)]
pub struct Headers {
    // TODO probably don't need Deref? It exposes HeaderMap impl and
    // probably shouldn't
    // NOT implementing DerefMut because mutators would need to also
    // clear the cache
    #[deref]
    headers: HeaderMap,
    // Cached sorted and combined list of header entries for iteration
    cached_iteration: RefCell<Option<Vec<(String, String)>>>,
}

// Sort and combine header entries, see:
// https://fetch.spec.whatwg.org/#concept-header-list-sort-and-combine
fn sort_and_combine_headers(headers: &HeaderMap) -> JsResult<Vec<(String, String)>> {
    // collect header entries into a BTreeMap to sort by header name
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for key in headers.keys() {
        let vals = Header::try_from_iter(headers.get_all(key))?;
        map.insert(key.to_string(), vals.headers);
    }

    // combine headers except for set-cookie
    let mut entries: Vec<(String, String)> = Vec::default();
    for (key, vals) in map.into_iter() {
        if key == "set-cookie" {
            for val in vals {
                entries.push((key.clone(), val));
            }
        } else {
            entries.push((key.clone(), vals.join(", ")))
        }
    }

    Ok(entries)
}

impl Headers {
    pub fn from_http_headers(
        headers: http::HeaderMap,
        _context: &mut Context<'_>,
    ) -> JsResult<Self> {
        Ok(Self {
            headers,
            cached_iteration: RefCell::default(),
        })
    }

    pub fn to_http_headers(&self) -> http::HeaderMap {
        self.headers.clone()
    }

    // clear cached iteration vector, should be called whenever we
    // modify the headers
    fn clear_cached_iteration(&self) {
        let mut cached_iteration = self.cached_iteration.borrow_mut();
        *cached_iteration = None;
    }

    // get (or rebuild) cached iteration vector
    fn get_cached_iteration(&self) -> JsResult<Vec<(String, String)>> {
        let mut cached_iteration = self.cached_iteration.borrow_mut();
        match cached_iteration.deref_mut() {
            Some(iterable) => Ok(iterable.clone()),
            None => {
                let iterable = sort_and_combine_headers(&self.headers)?;
                *cached_iteration = Some(iterable.clone());
                Ok(iterable)
            }
        }
    }
}

impl Finalize for Headers {}

unsafe impl Trace for Headers {
    empty_trace!();
}

impl From<HeaderMap> for Headers {
    fn from(headers: HeaderMap) -> Self {
        Self {
            headers,
            cached_iteration: RefCell::default(),
        }
    }
}

// A collection of header values
pub struct Header {
    pub headers: Vec<String>,
}

impl Header {
    pub fn try_from_iter<'a, T>(iter: T) -> JsResult<Self>
    where
        T: IntoIterator<Item = &'a HeaderValue>,
    {
        let headers = iter
            .into_iter()
            .map(|header_value| header_value.to_str().map(|x| x.into()))
            .collect::<Result<Vec<String>, http::header::ToStrError>>()
            .map_err(|_| {
                JsError::from_native(JsNativeError::typ().with_message(
                    "Failed to convert header value to printable ascii string",
                ))
            })?;
        Ok(Header { headers })
    }
}

impl IntoJs for Header {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        if self.headers.is_empty() {
            return JsValue::null();
        }
        self.headers.join(", ").into_js(context)
    }
}

impl Headers {
    /// Creates a new Headers object
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new value onto an existing header inside a Headers object, or adds the
    /// header if it does not already exist.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-append
    pub fn append(&mut self, name: &str, value: &str) -> JsResult<()> {
        self.clear_cached_iteration();
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
        self.clear_cached_iteration();
        let name = str_to_header_name(name)?;
        match self.headers.entry(name) {
            Entry::Occupied(entry) => {
                entry.remove_entry_mult();
                Ok(())
            }
            Entry::Vacant(_) => Ok(()),
        }
    }

    /// Returns a String of all the values of a header within a Headers object with a given name.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-headers-get
    pub fn get(&self, name: &str) -> JsResult<Header> {
        let name = str_to_header_name(name)?;
        let headers = Header::try_from_iter(self.headers.get_all(name))?;
        Ok(headers)
    }

    pub fn get_set_cookie(&self) -> JsResult<Vec<String>> {
        let headers = Header::try_from_iter(self.headers.get_all("set-cookie"))?;
        Ok(headers.headers)
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
        self.clear_cached_iteration();
        let name = str_to_header_name(name)?;
        let value = str_to_header_value(value)?;
        self.headers.insert(name, value);
        Ok(())
    }
}

pub struct HeadersClass;

impl Headers {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Headers`")
                    .into()
            })
    }
}

// hmm, this doesn't seem to belong here
fn require_args(
    args: &[JsValue],
    length: usize,
    method_name: &str,
    class_name: &str,
) -> JsResult<()> {
    if args.len() < length {
        return Err(JsNativeError::typ()
            .with_message(format!(
		"Failed to execute '{}' on '{}': {} arguments required, but only {} present.",
		method_name,
		class_name,
		length,
		args.len()
            ))
            .into());
    }
    Ok(())
}

#[derive(Deref)]
struct ByteString(String);

impl TryFromJs for ByteString {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let s: JsString = value.to_string(context)?;
        for c in s.iter() {
            if c >= &256 {
                return Err(JsNativeError::typ().with_message("invalid ByteString"))?;
            }
        }
        let string = s.to_std_string().ok().ok_or::<JsError>(
            JsNativeError::typ()
                .with_message("error converting value to String")
                .into(),
        )?;
        Ok(ByteString(string))
    }
}

#[derive(Deref)]
struct HeaderVal(String);

const HTTP_WHITESPACE: &[char] = &[
    '\u{09}', // HT
    '\u{20}', // SP
    '\u{0A}', // LF
    '\u{0D}', // CR
];

fn normalize_header_value(string: &str) -> &str {
    string
        .trim_start_matches(HTTP_WHITESPACE)
        .trim_end_matches(HTTP_WHITESPACE)
}

impl TryFromJs for HeaderVal {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let ByteString(string) = value.try_js_into(context)?;
        let string = normalize_header_value(string.as_str()).to_string();
        Ok(HeaderVal(string))
    }
}

impl HeadersClass {
    fn append(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        require_args(args, 2, "append", "Headers")?;
        let mut headers = Headers::try_from_js(this)?;
        let name: ByteString = args.get_or_undefined(0).try_js_into(context)?;
        let value: HeaderVal = args.get_or_undefined(1).try_js_into(context)?;

        headers.append(&name, &value)?;

        Ok(JsValue::undefined())
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        require_args(args, 1, "delete", "Headers")?;
        let mut headers = Headers::try_from_js(this)?;
        let name: ByteString = args.get_or_undefined(0).try_js_into(context)?;

        headers.remove(&name)?;

        Ok(JsValue::undefined())
    }

    fn get(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        require_args(args, 1, "get", "Headers")?;
        let headers = Headers::try_from_js(this)?;
        let name: ByteString = args.get_or_undefined(0).try_js_into(context)?;

        Ok(headers.get(&name)?.into_js(context))
    }

    fn get_set_cookie(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let headers = Headers::try_from_js(this)?;
        Ok(headers.get_set_cookie()?.into_js(context))
    }

    fn has(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        require_args(args, 1, "has", "Headers")?;
        let headers = Headers::try_from_js(this)?;
        let name: ByteString = args.get_or_undefined(0).try_js_into(context)?;

        Ok(headers.contains(&name)?.into())
    }

    fn set(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        require_args(args, 2, "set", "Headers")?;
        let mut headers = Headers::try_from_js(this)?;
        let name: ByteString = args.get_or_undefined(0).try_js_into(context)?;
        let value: HeaderVal = args.get_or_undefined(1).try_js_into(context)?;

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
        let obj = value.as_object()
	    .ok_or(JsNativeError::typ()
		   .with_message("Failed to construct 'Headers': The provided value cannot be converted to a sequence."))?;
        let arr: JsArray = iterable_to_sequence(obj, context)?.try_js_into(context)?;

        if arr.length(context)? != 2 {
            return Err(JsNativeError::typ().with_message(
                "Failed to construct 'Headers': sequence element with length not 2",
            ))?;
        }

        let name: ByteString = arr.get(0, context)?.try_js_into(context)?;
        let value: HeaderVal = arr.get(1, context)?.try_js_into(context)?;
        let name: String = name.to_string();
        let value: String = value.to_string();

        Ok(Self { name, value })
    }
}

fn get_iterator(obj: &JsObject, context: &mut Context<'_>) -> JsResult<JsValue> {
    // TODO workaround until JsSymbol::iterator() is pub
    let symbol_iterator: JsSymbol = context
        .intrinsics()
        .constructors()
        .symbol()
        .constructor()
        .get(js_string!("iterator"), context)?
        .as_symbol()
        .ok_or(JsNativeError::typ().with_message("Symbol.iterator was not a Symbol?"))?;
    obj.get(symbol_iterator, context)
}

// TODO this is not spec compliant, because it doesn't observe the
// object in the precisely the right way.
//
// Should do: https://webidl.spec.whatwg.org/#es-sequence
// for HeadersInit in particular we have sequence<sequence<ByteString>>
//
// TODO: using Array.from as a hack, expose something in boa
fn iterable_to_sequence(obj: &JsObject, context: &mut Context<'_>) -> JsResult<JsValue> {
    let array_from_obj = context
        .intrinsics()
        .constructors()
        .array()
        .constructor()
        .get(js_string!("from"), context)?;
    let array_from: &JsObject = array_from_obj.as_callable().ok_or::<JsError>(
        JsNativeError::typ()
            .with_message("Array.from was not callable")
            .into(),
    )?;

    if get_iterator(obj, context)?.is_callable() {
        array_from.call(&JsValue::null(), &[obj.clone().into()], context)
    } else {
        Err(JsNativeError::typ()
            .with_message("expected iterable to convert to sequence")
            .into())
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

/// The `HeadersInit` enum
///
/// More information:
///  - [WHATWG specification][spec]
///
/// [spec] https://fetch.spec.whatwg.org/#typedefdef-headersinit
#[derive(Default)]
pub struct HeadersInit {
    entries: Vec<HeaderEntry>,
}

impl Headers {
    pub fn from_init(init: HeadersInit) -> JsResult<Headers> {
        let mut headers = Headers::default();
        for entry in init.entries {
            headers.append(&entry.name, &entry.value)?
        }
        Ok(headers)
    }
}

impl TryFromJs for HeadersInit {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if value.is_undefined() {
            return Ok(HeadersInit::default());
        }

        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("Failed to convert js value into js object"),
            )
        })?;

        // if it looks iterable, convert to sequence<sequence<ByteString>>
        let entries = if get_iterator(obj, context)?.is_callable() {
            let entries_obj = iterable_to_sequence(obj, context)?;
            let entries_array = entries_obj
                .as_object()
                .ok_or(JsNativeError::typ().with_message(
                "Internal error in Headers constructor: expected array from `Array.from`",
            ))?;
            js_array_to_header_entries(entries_array, context)?
        } else {
            // TODO: Expose `enumerable_own_property_names` in Boa
            // TODO: also should throw if there are any own property
            // symbols, because we should try to ToString them
            let arr = builtins::object::Object::entries(
                &JsValue::undefined(),
                &[value.clone()],
                context,
            )?
            .to_object(context)
            .ok()
            .ok_or(
                JsNativeError::typ().with_message("Expected array from `Object.entries`"),
            )?;

            js_array_to_header_entries(&arr, context)?
        };

        Ok(HeadersInit { entries })
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
            Some(value) => {
                let init: HeadersInit = value.try_js_into(context)?;
                Headers::from_init(init)
            }
        }
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        class
            .method(
                js_string!("append"),
                2,
                NativeFunction::from_fn_ptr(HeadersClass::append),
            )
            .method(
                js_string!("delete"),
                1,
                NativeFunction::from_fn_ptr(HeadersClass::delete),
            )
            .method(
                js_string!("get"),
                1,
                NativeFunction::from_fn_ptr(HeadersClass::get),
            )
            .method(
                js_string!("getSetCookie"),
                0,
                NativeFunction::from_fn_ptr(HeadersClass::get_set_cookie),
            )
            .method(
                js_string!("has"),
                1,
                NativeFunction::from_fn_ptr(HeadersClass::has),
            )
            .method(
                js_string!("set"),
                2,
                NativeFunction::from_fn_ptr(HeadersClass::set),
            );
        PairIterableMethods::<HeadersIteratorClass>::define_pair_iterable_methods(class)?;
        Ok(())
    }
}

impl PairIterable for Headers {
    fn pair_iterable_len(&self) -> JsResult<usize> {
        Ok(self.get_cached_iteration()?.len())
    }

    fn pair_iterable_get(
        &self,
        index: usize,
        context: &mut Context<'_>,
    ) -> JsResult<jstz_core::iterators::PairValue> {
        let cached_iteration = self.get_cached_iteration()?;
        match cached_iteration.get(index) {
            None => Err(JsNativeError::typ()
                .with_message("Index out of bounds error in iterator"))?,
            Some(elem) => {
                let elem = elem.clone();
                let key: JsValue = elem.0.into_js(context);
                let value: JsValue = elem.1.into_js(context);
                Ok(PairValue { key, value })
            }
        }
    }
}

struct HeadersIteratorClass;
impl PairIteratorClass for HeadersIteratorClass {
    type Iterable = Headers;

    const NAME: &'static str = "Headers Iterator";
}

pub struct HeadersApi;

impl jstz_core::Api for HeadersApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<HeadersClass>(context)
            .expect("The `Headers` class shouldn't exist yet");
        register_global_class::<HeadersIteratorClass>(context)
            .expect("The `Headers Iterator` class shouldn't exist yet");
    }
}
