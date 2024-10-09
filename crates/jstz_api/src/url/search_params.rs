use std::fmt::{self, Display};

use boa_engine::{
    builtins, js_string,
    object::{builtins::JsArray, ErasedObject},
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsString,
    JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    iterators::{PairIterable, PairIterableMethods, PairIteratorClass, PairValue},
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject,
        JsNativeObjectToString, NativeClass,
    },
    value::IntoJs,
};

use super::Url;

pub type Name = String;
pub type Value = String;

/// `UrlSearchParams` is an object with utility methods that define
/// the query string of a `Url`.
///
/// [spec] https://url.spec.whatwg.org/#urlsearchparams
#[derive(JsData, Default)]
pub struct UrlSearchParams {
    values: Vec<(Name, Value)>,
    pub(crate) url: Option<JsNativeObject<Url>>,
}

impl Finalize for UrlSearchParams {}

unsafe impl Trace for UrlSearchParams {
    // Do not trace the `url` since this is only set if
    // there exists a parent `url` object (whose lifetime bounds the
    // search params object)
    empty_trace!();
}

impl JsNativeObjectToString for UrlSearchParams {
    fn to_string(
        this: &JsNativeObject<Self>,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        Ok(this.deref().to_string().into_js(context))
    }
}

impl UrlSearchParams {
    pub(crate) fn set_values(&mut self, values: Vec<(Name, Value)>) {
        self.values = values
    }

    pub(crate) fn set_url(&mut self, url: &JsNativeObject<Url>) {
        self.url = Some(url.clone());
    }

    pub fn new(values: Vec<(Name, Value)>) -> Self {
        Self { values, url: None }
    }

    /// Updates the query params of the associated `Url`
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#concept-urlsearchparams-update
    fn update(&self) {
        if let Some(url) = &self.url {
            let mut url = url.deref_mut();

            if self.values.is_empty() {
                url.url.set_query(None);
            } else {
                url.url.query_pairs_mut().clear().extend_pairs(&self.values);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// to satisfy clippy::len_without_is_empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Appends a specified key/value pair as a new search parameter.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-append
    pub fn append(&mut self, name: Name, value: Value) {
        // 1. Append `(name, value)` to `self`'s `values`
        self.values.push((name, value));
        // 2. Update `self`
        self.update();
    }

    /// Removes search parameters that match a name, and optional value, from the
    /// list of all search parameters.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-delete
    pub fn remove(&mut self, name: Name, value: Option<Value>) {
        // 1. If value is given,
        if let Some(value) = value {
            // 1. (cont.) Then removal all tuples whose name is `name` and value is `value`
            self.values.retain(|(k, v)| k != &name && v != &value)
        } else {
            // 2. Otherwise, removal all tuples whose name is `name`
            self.values.retain(|(k, _)| k != &name)
        }
        // 3. Update `self`
        self.update();
    }

    /// Returns the first value associated with the given search parameter.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-get
    pub fn get(&self, name: Name) -> Option<Value> {
        self.values
            .iter()
            .find(|(k, _)| k == &name)
            .map(|(_, v)| v.clone())
    }

    /// Returns all the values associated with a given search parameter.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-getall
    pub fn get_all(&self, name: Name) -> Vec<Value> {
        self.values
            .iter()
            .filter(|(k, _)| k == &name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Returns a boolean value indicating if a given parameter, or
    /// parameter and value pair, exists.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-has
    pub fn contains(&self, name: Name, value: Option<Value>) -> bool {
        // 1. If value is given,
        if let Some(value) = value {
            // 1. (cont.) and there is a tuple whose name is `name` and value is `value`
            //    in `self`'s `values`, then return true
            // 3. Otherwise, return false
            self.values.iter().any(|(k, v)| k == &name && v == &value)
        } else {
            // 2. If value is not given and there is a tuple whose name is `name` in
            //    in `self`'s `values`, then return true
            // 3. Otherwise, return false
            self.values.iter().any(|(k, _)| k == &name)
        }
    }

    /// Sets the value associated with a given search parameter to the given
    /// value. If there are several values, the others are deleted.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-set
    pub fn set(&mut self, name: Name, value: Value) {
        // 1. If `self`'s `values` contains any tuples whose name is `name`, then set
        //    the value of the first such tuple to value and remove others
        let mut i = 0;
        let mut slot = None;
        self.values.retain_mut(|(k, _)| {
            if slot.is_none() {
                if k == &name {
                    slot = Some(i)
                } else {
                    i += 1
                };
                true
            } else {
                k != &name
            }
        });

        match slot {
            Some(i) => self.values[i].1 = value,
            None => {
                // 2. Otherwise, append `(name, value)` to `self`'s `values`
                self.values.push((name, value))
            }
        }

        // 3. Update `self`
        self.update()
    }

    /// Sorts all key/values pairs, if any, by their keys.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-urlsearchparams-sort
    pub fn sort(&mut self) {
        // 1. Sort all tuples in `self`'s `values`, if any, by their names
        //    Sorting must be done by comparisong of code units
        //    The releative order between tuples with equal names must be preserved
        self.values
            .sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));

        // 2. Update `self`
        self.update()
    }
}

impl Display for UrlSearchParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.values
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<String>>()
                .join("&")
        )
    }
}

pub enum UrlSearchParamsInit {
    Object(JsObject),
    Array(JsObject),
    String(JsString),
}

fn js_array_into_url_search_params_values(
    obj: JsObject,
    context: &mut Context,
) -> JsResult<Vec<(Name, Value)>> {
    let arr = JsArray::from_object(obj)?;

    let mut vec = vec![];

    let length = arr.length(context)?;
    for i in 0..length {
        let arr: JsArray = arr.get(i, context)?.try_js_into(context)?;

        let name: Name = arr.get(0, context)?.try_js_into(context)?;
        let value: Value = arr.get(1, context)?.try_js_into(context)?;

        vec.push((name, value))
    }

    Ok(vec)
}

impl UrlSearchParams {
    fn parse(params: &str) -> Vec<(Name, Value)> {
        form_urlencoded::parse(params.as_bytes())
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn from_init(init: UrlSearchParamsInit, context: &mut Context) -> JsResult<Self> {
        match init {
            UrlSearchParamsInit::Object(obj) => {
                let arr = builtins::object::OrdinaryObject::entries(
                    &JsValue::undefined(),
                    &[obj.into()],
                    context,
                )?
                .to_object(context)
                .expect("Expected array from `Object.entries`");

                Ok(Self::new(js_array_into_url_search_params_values(
                    arr, context,
                )?))
            }
            UrlSearchParamsInit::Array(arr) => {
                let values = js_array_into_url_search_params_values(arr, context)?;

                Ok(Self::new(values))
            }
            UrlSearchParamsInit::String(string) => {
                let values = Self::parse(string.to_std_string_escaped().as_str());

                Ok(Self::new(values))
            }
        }
    }
}

impl TryFromJs for UrlSearchParamsInit {
    fn try_from_js(value: &JsValue, _context: &mut Context) -> JsResult<Self> {
        if let Some(string) = value.as_string() {
            Ok(Self::String(string.clone()))
        } else {
            let obj = value.as_object().ok_or_else(|| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Failed to convert js value into js object"),
                )
            })?;

            if obj.is_array() {
                Ok(Self::Array(obj.clone()))
            } else {
                Ok(Self::Object(obj.clone()))
            }
        }
    }
}

impl TryFromJs for UrlSearchParams {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let init: UrlSearchParamsInit = value.try_js_into(context)?;

        Self::from_init(init, context)
    }
}

pub struct UrlSearchParamsClass;

impl UrlSearchParams {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `UrlSearchParams`",
                    )
                    .into()
            })
    }
}

impl UrlSearchParamsClass {
    fn size(context: &mut Context) -> Accessor {
        accessor!(
            context,
            UrlSearchParams,
            "size",
            get:((search_params, _context) => Ok(search_params.len().into()))
        )
    }

    fn append(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: String = args.get_or_undefined(1).try_js_into(context)?;

        search_params.append(name, value);

        Ok(JsValue::undefined())
    }

    fn delete(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: Option<String> = args.get_or_undefined(1).try_js_into(context)?;

        search_params.remove(name, value);

        Ok(JsValue::undefined())
    }

    fn get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;

        match search_params.get(name) {
            Some(value) => Ok(value.into_js(context)),
            None => Ok(JsValue::null()),
        }
    }

    fn get_all(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;

        let values: Vec<JsValue> = search_params
            .get_all(name)
            .into_iter()
            .map(|value| value.into_js(context))
            .collect();

        Ok(JsArray::from_iter(values, context).into())
    }

    fn has(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: Option<String> = args.get_or_undefined(1).try_js_into(context)?;

        Ok(search_params.contains(name, value).into())
    }

    fn set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        let mut search_params = UrlSearchParams::try_from_js(this)?;
        let name: String = args.get_or_undefined(0).try_js_into(context)?;
        let value: String = args.get_or_undefined(1).try_js_into(context)?;

        search_params.set(name, value);

        Ok(JsValue::undefined())
    }

    fn sort(
        this: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut search_params = UrlSearchParams::try_from_js(this)?;

        search_params.sort();

        Ok(JsValue::undefined())
    }
}

impl NativeClass for UrlSearchParamsClass {
    type Instance = UrlSearchParams;

    const NAME: &'static str = "URLSearchParams";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<UrlSearchParams> {
        match args.first() {
            None => Ok(UrlSearchParams::default()),
            Some(init) => init.try_js_into(context),
        }
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        let size = UrlSearchParamsClass::size(class.context());

        class
            .accessor(js_string!("size"), size, Attribute::all())
            .method(
                js_string!("append"),
                1,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::append),
            )
            .method(
                js_string!("delete"),
                1,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::delete),
            )
            .method(
                js_string!("get"),
                1,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::get),
            )
            .method(
                js_string!("getAll"),
                1,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::get_all),
            )
            .method(
                js_string!("has"),
                1,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::has),
            )
            .method(
                js_string!("set"),
                2,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::set),
            )
            .method(
                js_string!("sort"),
                0,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::sort),
            )
            .method(
                js_string!("toString"),
                0,
                NativeFunction::from_fn_ptr(UrlSearchParamsClass::to_string),
            );

        PairIterableMethods::<UrlSearchParamsIteratorClass>::define_pair_iterable_methods(class)?;

        Ok(())
    }
}

impl PairIterable for UrlSearchParams {
    fn pair_iterable_len(&self) -> JsResult<usize> {
        Ok(self.values.len())
    }

    fn pair_iterable_get(
        &self,
        index: usize,
        context: &mut Context,
    ) -> JsResult<PairValue> {
        let pair = self.values.get(index).ok_or::<JsError>(
            JsNativeError::typ()
                .with_message("index out of bounds in UrlSearchParams Iterator")
                .into(),
        )?;
        let key = pair.0.clone().into_js(context);
        let value = pair.1.clone().into_js(context);
        Ok(PairValue { key, value })
    }
}

struct UrlSearchParamsIteratorClass;

impl PairIteratorClass for UrlSearchParamsIteratorClass {
    type Iterable = UrlSearchParams;
    const NAME: &'static str = "URLSearchParams Iterator";
}

pub struct UrlSearchParamsApi;

impl jstz_core::Api for UrlSearchParamsApi {
    fn init(self, context: &mut Context) {
        register_global_class::<UrlSearchParamsClass>(context)
            .expect("The `URLSearchParams` class shouldn't exist yet");
        // TODO should not really be a global class, remove from
        // global object when possible
        register_global_class::<UrlSearchParamsIteratorClass>(context)
            .expect("The `URLSearchParams Iterator` class shouldn't exist yet");
    }
}
