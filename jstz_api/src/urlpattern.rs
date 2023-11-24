//! `jstz`'s implementation of JavaScript's `URLPattern` Web API.
//!
//! FIXME: This is not spec compliant yet
//!        (see FIXME above `new` for more details)
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `URLPattern` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/URLPattern
//! [spec]: https://urlpattern.spec.whatwg.org/

use boa_engine::{
    js_string,
    object::{builtins::JsArray, Object},
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
};
use boa_gc::{empty_trace, Finalize, GcRefMut, Trace};

use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject,
        JsNativeObjectToString, NativeClass,
    },
    value::IntoJs,
};

use urlpattern::{
    quirks, UrlPattern as InnerUrlPattern,
    UrlPatternComponentResult as InnerUrlPatternComponentResult,
    UrlPatternMatchInput as InnerUrlPatternMatchInput,
    UrlPatternResult as InnerUrlPatternResult,
};

pub struct UrlPatternInput(quirks::StringOrInit);

impl Default for UrlPatternInput {
    fn default() -> Self {
        Self(quirks::StringOrInit::Init(quirks::UrlPatternInit::default()))
    }
}
#[derive(Default)]
pub struct UrlPatternInit(quirks::UrlPatternInit);

pub struct UrlPatternComponentResult(InnerUrlPatternComponentResult);

pub struct UrlPatternResult {
    pub(crate) inputs: Vec<UrlPatternInput>,
    pub(crate) result: InnerUrlPatternResult,
}

#[derive(Finalize)]
pub struct UrlPattern {
    pub(crate) url_pattern: InnerUrlPattern,
}

unsafe impl Trace for UrlPattern {
    empty_trace!();
}

impl JsNativeObjectToString for UrlPattern {
    fn to_string(
        this: &JsNativeObject<Self>,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let s = format!("{:?}", this.deref().url_pattern);
        Ok(s.into_js(context))
    }
}

impl UrlPattern {
    fn process_input(
        input: UrlPatternInput,
        base_url: Option<String>,
    ) -> JsResult<(
        InnerUrlPatternMatchInput,
        (quirks::StringOrInit, Option<String>),
    )> {
        match quirks::process_match_input(input.0, base_url.as_deref()) {
            Err(e) => Err(JsError::from_native(
                JsNativeError::typ().with_message(e.to_string()),
            )),
            Ok(Some(input)) => Ok(input),
            Ok(None) => Err(JsError::from_native(JsNativeError::error())),
        }
    }

    // FIXME: We do not support options (ignoreCase), as it is not supported in Deno
    // nor in `urlpattern` crate. There is an open PR for supporting it in
    // `urlpattern`: https://github.com/denoland/rust-urlpattern/pull/34
    pub fn new(
        _this: &JsNativeObject<Self>,
        input: UrlPatternInput,
        base_url: Option<String>,
        _context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let url_pattern_init =
            quirks::process_construct_pattern_input(input.0, base_url.as_deref())
                .map_err(|_| {
                    JsError::from_native(
                        JsNativeError::typ()
                            .with_message("Failed to build UrlPatternInit"),
                    )
                })?;
        let url_pattern = InnerUrlPattern::parse(url_pattern_init).map_err(|_| {
            JsError::from_native(
                JsNativeError::typ().with_message("Failed to parse UrlPatternInit"),
            )
        })?;

        Ok(Self { url_pattern })
    }

    pub fn protocol(&self) -> String {
        String::from(self.url_pattern.protocol())
    }

    pub fn username(&self) -> String {
        String::from(self.url_pattern.username())
    }

    pub fn password(&self) -> String {
        String::from(self.url_pattern.password())
    }

    pub fn hostname(&self) -> String {
        String::from(self.url_pattern.hostname())
    }

    pub fn port(&self) -> String {
        String::from(self.url_pattern.port())
    }

    pub fn pathname(&self) -> String {
        String::from(self.url_pattern.pathname())
    }

    pub fn search(&self) -> String {
        String::from(self.url_pattern.search())
    }

    pub fn hash(&self) -> String {
        String::from(self.url_pattern.hash())
    }

    pub fn test(
        &self,
        input: UrlPatternInput,
        base_url: Option<String>,
    ) -> JsResult<bool> {
        let (url_pattern_match_input, _) = Self::process_input(input, base_url)?;

        self.url_pattern
            .test(url_pattern_match_input)
            .map_err(|e| JsNativeError::typ().with_message(e.to_string()).into())
    }

    pub fn exec(
        &self,
        input: UrlPatternInput,
        base_url: Option<String>,
    ) -> JsResult<Option<UrlPatternResult>> {
        let (url_pattern_match_input, (string_or_init, base_url)) =
            Self::process_input(input, base_url)?;

        let mut inputs: Vec<UrlPatternInput> = Vec::new();
        inputs.push(UrlPatternInput(string_or_init));
        if let Some(base_url) = base_url {
            inputs.push(UrlPatternInput::from(base_url));
        }
        self.url_pattern
            .exec(url_pattern_match_input)
            .map(|op| op.map(|result| UrlPatternResult { inputs, result }))
            .map_err(|e| JsNativeError::typ().with_message(e.to_string()).into())
    }
}

pub struct UrlPatternClass;

impl UrlPattern {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message(
                        "Failed to convert js value into rust type `UrlPattern`",
                    )
                    .into()
            })
    }
}

impl UrlPatternClass {
    fn hash(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "hash",
            get:((url, context) => Ok(url.hash().into_js(context)))
        )
    }

    fn hostname(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "hostname",
            get:((url, context) => Ok(url.hostname().into_js(context)))
        )
    }

    fn password(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "password",
            get:((url, context) => Ok(url.password().into_js(context)))
        )
    }

    fn pathname(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "pathname",
            get:((url, context) => Ok(url.pathname().into_js(context)))
        )
    }

    fn port(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "port",
            get:((url, context) => Ok(url.port().into_js(context)))
        )
    }

    fn protocol(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "protocol",
            get:((url, context) => Ok(url.protocol().into_js(context)))
        )
    }

    fn search(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "search",
            get:((url, context) => Ok(url.search().into_js(context)))
        )
    }

    fn username(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            UrlPattern,
            "username",
            get:((url, context) => Ok(url.username().into_js(context)))
        )
    }

    fn test(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let url_pattern = UrlPattern::try_from_js(this)?;
        let input: UrlPatternInput = match args.get(0) {
            Some(value) => value.try_js_into(context)?,
            None => UrlPatternInput::default(),
        };
        let base_url: Option<String> = args.get_or_undefined(1).try_js_into(context).ok();
        Ok(url_pattern.test(input, base_url)?.into_js(context))
    }

    fn exec(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let url_pattern = UrlPattern::try_from_js(this)?;
        let input: UrlPatternInput = match args.get(0) {
            Some(value) => value.try_js_into(context)?,
            None => UrlPatternInput::default(),
        };
        let base_url: Option<String> = args.get_or_undefined(1).try_js_into(context).ok();
        url_pattern
            .exec(input, base_url)?
            .map_or(Ok(JsValue::Null), |r| Ok(r.into_js(context)))
    }
}

impl TryFromJs for UrlPatternInit {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected `JsObject`"))
        })?;

        macro_rules! get_optional_property {
            ($obj:ident, $field:literal, $context:ident) => {
                if $obj.has_property(js_string!($field), $context)? {
                    $obj.get(js_string!($field), $context)?
                        .try_js_into($context)?
                } else {
                    None
                }
            };
        }

        let url_pattern_init = quirks::UrlPatternInit {
            protocol: get_optional_property!(obj, "protocol", context),
            username: get_optional_property!(obj, "username", context),
            password: get_optional_property!(obj, "password", context),
            hostname: get_optional_property!(obj, "hostname", context),
            port: get_optional_property!(obj, "port", context),
            pathname: get_optional_property!(obj, "pathname", context),
            search: get_optional_property!(obj, "search", context),
            hash: get_optional_property!(obj, "hash", context),
            base_url: get_optional_property!(obj, "baseURL", context),
        };

        Ok(Self(url_pattern_init))
    }
}

impl TryFromJs for UrlPatternInput {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if value.is_string() {
            let string: String = value.try_js_into(context)?;
            return Ok(Self(quirks::StringOrInit::String(string)));
        };

        let UrlPatternInit(init) = UrlPatternInit::try_from_js(value, context)?;
        Ok(Self(quirks::StringOrInit::Init(init)))
    }
}

impl IntoJs for UrlPatternInput {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let UrlPatternInput(string_or_init) = self;
        match string_or_init {
            quirks::StringOrInit::Init(init) => UrlPatternInit(init).into_js(context),
            quirks::StringOrInit::String(string) => string.into_js(context),
        }
    }
}

impl From<String> for UrlPatternInput {
    fn from(value: String) -> Self {
        UrlPatternInput(quirks::StringOrInit::String(value))
    }
}

impl From<quirks::UrlPatternInit> for UrlPatternInput {
    fn from(value: quirks::UrlPatternInit) -> Self {
        UrlPatternInput(quirks::StringOrInit::Init(value))
    }
}

impl IntoJs for UrlPatternComponentResult {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let url_pattern_component_result = self.0;
        let input = url_pattern_component_result.input;
        let groups: Vec<(String, String)> =
            url_pattern_component_result.groups.into_iter().collect();
        // Create an object with prototype set to `Object.prototype`
        let obj = JsObject::with_object_proto(context.intrinsics());
        // Add data property `input` to the object
        // TODO: Support error handling (using TryIntoJs)
        let _ = obj.create_data_property(
            js_string!("input"),
            input.into_js(context),
            context,
        );
        // Add data property `groups` to the object,
        // which is itself another object with prototype set to `Object.prototype`
        let group_obj = JsObject::with_object_proto(context.intrinsics());
        for (key, value) in groups.iter() {
            let value = value.clone().into_js(context);

            // TODO: Support error handling (using TryIntoJs)
            let _ =
                group_obj.create_data_property(js_string!(key.clone()), value, context);
        }

        // TODO: Support error handling (using TryIntoJs)
        let _ = obj.create_data_property(js_string!("groups"), group_obj, context);
        obj.into()
    }
}

impl IntoJs for UrlPatternInit {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let obj = JsObject::with_object_proto(context.intrinsics());
        let init = self.0;

        macro_rules! create_data_properties_if_some {
            ($obj:ident, $init:ident, $field:ident, $context:ident) => {
                if let Some(s) = $init.$field {
                    // TODO: Support error handling (using TryIntoJs)
                    let _ = $obj.create_data_property(
                        js_string!(stringify!($field)),
                        js_string!(s),
                        $context,
                    );
                }
            };
            ($obj:ident, $init:ident, $field:ident, $property_name:literal, $context:ident) => {
                if let Some(s) = $init.$field {
                    // TODO: Support error handling (using TryIntoJs)
                    let _ = $obj.create_data_property(
                        js_string!($property_name),
                        js_string!(s),
                        $context,
                    );
                }
            };
        }

        create_data_properties_if_some!(obj, init, protocol, context);
        create_data_properties_if_some!(obj, init, username, context);
        create_data_properties_if_some!(obj, init, password, context);
        create_data_properties_if_some!(obj, init, hostname, context);
        create_data_properties_if_some!(obj, init, port, context);
        create_data_properties_if_some!(obj, init, pathname, context);
        create_data_properties_if_some!(obj, init, search, context);
        create_data_properties_if_some!(obj, init, hash, context);
        create_data_properties_if_some!(obj, init, base_url, "baseURL", context);

        obj.into()
    }
}

impl IntoJs for UrlPatternResult {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let UrlPatternResult { result, inputs } = self;
        let obj = JsObject::with_object_proto(context.intrinsics());

        macro_rules! create_data_property {
            ($obj:ident, $inner:ident, $field:ident, $context:ident) => {
                let $field = UrlPatternComponentResult($inner.$field).into_js($context);
                let _ = $obj.create_data_property(
                    js_string!(stringify!($field)),
                    $field,
                    $context,
                );
            };
        }

        create_data_property!(obj, result, protocol, context);
        create_data_property!(obj, result, username, context);
        create_data_property!(obj, result, password, context);
        create_data_property!(obj, result, hostname, context);
        create_data_property!(obj, result, port, context);
        create_data_property!(obj, result, pathname, context);
        create_data_property!(obj, result, search, context);
        create_data_property!(obj, result, hash, context);

        let inputs: JsValue = {
            let array: JsArray = JsArray::new(context);
            for input in inputs.into_iter() {
                let _ = array.push(input.into_js(context), context);
            }
            array.into()
        };
        let _ = obj.create_data_property(js_string!("inputs"), inputs, context);

        obj.into()
    }
}

impl NativeClass for UrlPatternClass {
    type Instance = UrlPattern;

    const NAME: &'static str = "URLPattern";

    fn constructor(
        this: &JsNativeObject<UrlPattern>,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<UrlPattern> {
        let input: UrlPatternInput = match args.get(0) {
            Some(value) => value.try_js_into(context)?,
            None => UrlPatternInput::default(),
        };
        let base_url: Option<String> = args.get_or_undefined(1).try_js_into(context)?;

        UrlPattern::new(this, input, base_url, context)
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let hash = UrlPatternClass::hash(class.context());
        let hostname = UrlPatternClass::hostname(class.context());
        let password = UrlPatternClass::password(class.context());
        let pathname = UrlPatternClass::pathname(class.context());
        let port = UrlPatternClass::port(class.context());
        let protocol = UrlPatternClass::protocol(class.context());
        let search = UrlPatternClass::search(class.context());
        let username = UrlPatternClass::username(class.context());

        class
            .accessor(js_string!("hash"), hash, Attribute::all())
            .accessor(js_string!("hostname"), hostname, Attribute::all())
            .accessor(js_string!("password"), password, Attribute::all())
            .accessor(js_string!("pathname"), pathname, Attribute::all())
            .accessor(js_string!("port"), port, Attribute::all())
            .accessor(js_string!("protocol"), protocol, Attribute::all())
            .accessor(js_string!("search"), search, Attribute::all())
            .accessor(js_string!("username"), username, Attribute::all())
            .method(
                js_string!("test"),
                0,
                NativeFunction::from_fn_ptr(UrlPatternClass::test),
            )
            .method(
                js_string!("exec"),
                0,
                NativeFunction::from_fn_ptr(UrlPatternClass::exec),
            );
        Ok(())
    }
}

pub struct UrlPatternApi;

impl jstz_core::Api for UrlPatternApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<UrlPatternClass>(context)
            .expect("The `URLPattern` class shouldn't exist yet")
    }
}

/*

Some tests adapted from Deno.

>> (function () {
  const pattern = new URLPattern("https://deno.land/foo/:bar");
  console.log(pattern.protocol == "https");
  console.log(pattern.protocol == "https");
  console.log(pattern.hostname == "deno.land");
  console.log(pattern.pathname == "/foo/:bar");

  console.log(pattern.test("https://deno.land/foo/x"));
  console.log(!pattern.test("https://deno.com/foo/x"));
  match = pattern.exec("https://deno.land/foo/x");
  console.log(match);
  console.log(match.pathname.input == "/foo/x");
  // Use `JSON.stringify` for simple comparison of objects
  console.log(JSON.stringify(match.pathname.groups) == JSON.stringify({ bar: "x" }));

})();
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] [object Object]
[🪵] true
[🪵] true

>> (function () {
  const pattern = new URLPattern("/foo/:bar", "https://deno.land");
  console.log(pattern.protocol == "https");
  console.log(pattern.hostname == "deno.land");
  console.log(pattern.pathname == "/foo/:bar");

  console.log(pattern.test("https://deno.land/foo/x"));
  console.log(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  console.log(match);
  console.log(match.pathname.input == "/foo/x");
  console.log(JSON.stringify(match.pathname.groups) == JSON.stringify({ bar: "x" }));
})();
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] [object Object]
[🪵] true
[🪵] true

>> (function () {
  const pattern = new URLPattern({
    pathname: "/foo/:bar",
  });
  console.log(pattern.protocol == "*");
  console.log(pattern.hostname == "*");
  console.log(pattern.pathname == "/foo/:bar");

  console.log(pattern.test("https://deno.land/foo/x"));
  console.log(pattern.test("https://deno.com/foo/x"));
  console.log(!pattern.test("https://deno.com/bar/x"));

  console.log(pattern.test({ pathname: "/foo/x" }));
})();
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true
[🪵] true

*/
