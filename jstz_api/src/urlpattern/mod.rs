//! `jstz`'s implementation of JavaScript's `URLPattern` Web API.
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
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{custom_trace, Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject,
        JsNativeObjectToString, NativeClass,
    },
    value::IntoJs,
};
use urlpattern::quirks::StringOrInit as InnerStringOrQuirksInit;
use urlpattern::quirks::UrlPatternInit as InnerUrlPatternQuirksInit;
use urlpattern::UrlPattern as InnerUrlPattern;
use urlpattern::UrlPatternComponentResult as InnerUrlPatternComponentResult;
use urlpattern::UrlPatternResult as InnerUrlPatternResult;

pub struct UrlPatternInput(InnerStringOrQuirksInit);
#[derive(Default)]
pub struct UrlPatternInit(InnerUrlPatternQuirksInit);

pub struct UrlPatternComponentResult(InnerUrlPatternComponentResult);
pub struct UrlPatternResult {
    // It should be UrlPatternInit instead of UrlPatternInput
    // according to Deno types?
    pub(crate) inputs: Vec<UrlPatternInput>,
    pub(crate) url_pattern_result: InnerUrlPatternResult,
}

#[derive(Finalize)]
pub struct UrlPattern {
    pub(crate) url_pattern: InnerUrlPattern,
}

unsafe impl Trace for UrlPattern {
    custom_trace!(_this, {});
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
    // We do not support options (ignoreCase), as it is not supported in Deno
    // nor in `urlpattern` crate. There is an open PR for supporting it in
    // `urlpattern`, and we could use it when it gets merged.
    pub fn new(
        _this: &JsNativeObject<Self>,
        input: UrlPatternInput,
        base_url: Option<String>,
        _context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let UrlPatternInput(stringorinit) = input;
        let urlpatterninit = urlpattern::quirks::process_construct_pattern_input(
            stringorinit,
            base_url.as_deref(),
        )
        .map_err(|_| {
            JsError::from_native(
                JsNativeError::typ().with_message("Failed to build UrlPatternInit"),
            )
        })?;
        let url_pattern = InnerUrlPattern::parse(urlpatterninit).map_err(|_| {
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
        let UrlPatternInput(string_or_init) = input;
        let (url_pattern_match_input, _) =
            urlpattern::quirks::process_match_input(string_or_init, base_url.as_deref())
                .unwrap()
                .unwrap();

        self.url_pattern.test(url_pattern_match_input).map_err(|_| {
            JsNativeError::typ()
                .with_message("Failed to run `test` on `UrlPattern`")
                .into()
        })
    }

    pub fn exec(
        &self,
        input: UrlPatternInput,
        base_url: Option<String>,
    ) -> JsResult<Option<UrlPatternResult>> {
        let UrlPatternInput(string_or_init) = input;
        let (url_pattern_match_input, (string_or_init, base_url)) =
            urlpattern::quirks::process_match_input(string_or_init, base_url.as_deref())
                .unwrap()
                .unwrap();
        let mut inputs: Vec<UrlPatternInput> = Vec::new();
        inputs.push(UrlPatternInput(string_or_init));
        if let Some(base_url) = base_url {
            inputs.push(UrlPatternInput(InnerStringOrQuirksInit::String(base_url)));
        }
        self.url_pattern
            .exec(url_pattern_match_input)
            .map(|op| {
                op.map(|url_pattern_result| UrlPatternResult {
                    inputs,
                    url_pattern_result,
                })
            })
            .map_err(|_| {
                JsNativeError::typ()
                    .with_message("Failed to run `exec` on `UrlPattern`")
                    .into()
            })
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
        let input: UrlPatternInput = args.get(0).unwrap().try_js_into(context)?;
        let base_url: Option<String> = args.get_or_undefined(1).try_js_into(context).ok();
        Ok(url_pattern.test(input, base_url)?.into_js(context))
    }

    fn exec(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let url_pattern = UrlPattern::try_from_js(this)?;
        let input: UrlPatternInput = args.get(0).unwrap().try_js_into(context)?;
        let base_url: Option<String> = args.get_or_undefined(1).try_js_into(context).ok();
        url_pattern
            .exec(input, base_url)?
            .map_or(Ok(JsValue::Null), |e| Ok(e.into_js(context)))
    }
}

impl TryFromJs for UrlPatternInit {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if value.is_undefined() {
            return Ok(UrlPatternInit::default());
        }

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

        let url_pattern_init = urlpattern::quirks::UrlPatternInit {
            protocol: get_optional_property!(obj, "protocol", context),
            username: get_optional_property!(obj, "username", context),
            password: get_optional_property!(obj, "password", context),
            hostname: get_optional_property!(obj, "hostname", context),
            port: get_optional_property!(obj, "port", context),
            pathname: get_optional_property!(obj, "pathname", context),
            search: get_optional_property!(obj, "search", context),
            hash: get_optional_property!(obj, "hash", context),
            base_url: get_optional_property!(obj, "base_url", context),
        };

        Ok(Self(url_pattern_init))
    }
}

impl TryFromJs for UrlPatternInput {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if let Some(string) = value.as_string() {
            return Ok(Self(InnerStringOrQuirksInit::String(
                string.to_std_string_escaped(),
            )));
        };

        let UrlPatternInit(init) = UrlPatternInit::try_from_js(value, context)?;
        Ok(Self(InnerStringOrQuirksInit::Init(init)))
    }
}

impl IntoJs for UrlPatternInput {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let UrlPatternInput(string_or_init) = self;
        match string_or_init {
            InnerStringOrQuirksInit::Init(init) => UrlPatternInit(init).into_js(context),
            InnerStringOrQuirksInit::String(string) => JsString::from(string).into(),
        }
    }
}

impl IntoJs for UrlPatternComponentResult {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let UrlPatternComponentResult(url_pattern_component_result) = self;
        let input = url_pattern_component_result.input;
        let groups: Vec<(String, String)> =
            url_pattern_component_result.groups.into_iter().collect();
        let obj = JsObject::with_object_proto(context.intrinsics());
        let _ = obj.create_data_property(
            JsString::from("input"),
            JsValue::String(JsString::from(input)),
            context,
        );
        let group_obj = JsObject::with_object_proto(context.intrinsics());
        for (key, value) in groups.iter() {
            let value = JsValue::String(JsString::from(value.clone()));
            let _ = group_obj.create_data_property(
                JsString::from(key.clone()),
                value,
                context,
            );
        }
        let _ = obj.create_data_property(JsString::from("groups"), group_obj, context);
        obj.into()
    }
}

impl IntoJs for UrlPatternInit {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let obj = JsObject::with_object_proto(context.intrinsics());
        let UrlPatternInit(init) = self;

        macro_rules! create_data_properties_if_some {
            ($obj:ident, $init:ident, $field:ident, $context:ident) => {
                let property_name = stringify!($field);
                if let Some(s) = $init.$field {
                    let _ = $obj.create_data_property(
                        JsString::from(property_name),
                        JsString::from(s),
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

        obj.into()
    }
}

impl IntoJs for UrlPatternResult {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        let UrlPatternResult {
            url_pattern_result,
            inputs,
        } = self;
        let obj = JsObject::with_object_proto(context.intrinsics());

        macro_rules! create_data_property {
            ($obj:ident, $inner:ident, $field:ident, $context:ident) => {
                let property_name = stringify!($field);
                let $field = UrlPatternComponentResult($inner.$field).into_js($context);
                let _ = $obj.create_data_property(
                    JsString::from(property_name),
                    $field,
                    $context,
                );
            };
        }

        create_data_property!(obj, url_pattern_result, protocol, context);
        create_data_property!(obj, url_pattern_result, username, context);
        create_data_property!(obj, url_pattern_result, password, context);
        create_data_property!(obj, url_pattern_result, hostname, context);
        create_data_property!(obj, url_pattern_result, port, context);
        create_data_property!(obj, url_pattern_result, pathname, context);
        create_data_property!(obj, url_pattern_result, search, context);
        create_data_property!(obj, url_pattern_result, hash, context);

        let inputs: JsValue = {
            let array: JsArray = JsArray::new(context);
            for input in inputs.into_iter() {
                let _ = array.push(input.into_js(context), context);
            }
            array.into()
        };
        let _ = obj.create_data_property(JsString::from("inputs"), inputs, context);

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
        let input: UrlPatternInput = args.get_or_undefined(0).try_js_into(context)?;
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

Some tests from Deno:

(function () {
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
  // false, but also false in Deno/Chrome
  console.log(match.pathname.groups == { bar: "x" });

})();

(function () {
  const pattern = new URLPattern("/foo/:bar", "https://deno.land");
  console.log(pattern.protocol == "https");
  console.log(pattern.hostname == "deno.land");
  console.log(pattern.pathname == "/foo/:bar");

  console.log(pattern.test("https://deno.land/foo/x"));
  console.log(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  console.log(match);
  console.log(match.pathname.input == "/foo/x");
  // false, but also false in Deno/Chrome
  console.log(match.pathname.groups == { bar: "x" });
})();

(function () {
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

*/
