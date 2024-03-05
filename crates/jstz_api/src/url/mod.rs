//! `jstz`'s implementation of JavaScript's `URL` Web API.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `URL` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/URL_API
//! [spec]: https://url.spec.whatwg.org/

mod search_params;

use std::{cmp::Ordering, ops::Deref};

use boa_engine::{
    js_string, object::Object, property::Attribute, Context, JsArgs, JsError,
    JsNativeError, JsObject, JsResult, JsValue, NativeFunction,
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
use url::Url as InnerUrl;

pub use search_params::UrlSearchParams;

use self::search_params::{UrlSearchParamsApi, UrlSearchParamsClass};

pub struct Url {
    pub(crate) url: InnerUrl,
    search_params: JsNativeObject<UrlSearchParams>,
}

impl Finalize for Url {
    fn finalize(&self) {
        self.search_params.deref().finalize()
    }
}

unsafe impl Trace for Url {
    custom_trace!(this, {
        mark(this.search_params.deref().deref());
    });
}

impl JsNativeObjectToString for Url {
    fn to_string(
        this: &JsNativeObject<Self>,
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        Ok(this.deref().href().into_js(context))
    }
}

impl Url {
    fn parse_url(url: String, base: Option<String>) -> Option<InnerUrl> {
        let options = InnerUrl::options();
        let base = base.as_ref().and_then(|base| InnerUrl::parse(base).ok());
        options.base_url(base.as_ref()).parse(&url).ok()
    }

    /// Creates and returns a URL object referencing the URL specified using an absolute
    /// URL string `url`, or a relative URL string `url` and a `base` URL string.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-url
    pub fn new(
        url: String,
        base: Option<String>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        // 1. Let `parsed_url` be the result of running the API URL parser on url with base, if given
        // 2. If `parsed_url` is failure, then throw a `TypeError`
        let parsed_url = Self::parse_url(url, base).ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Failed to parse url"))
        })?;

        // 3. Let `query` be `parsed_url`’s query, if that is non-null, and the empty list otherwise
        let query = parsed_url.query_pairs().into_owned().collect();

        // 4. Set `url`’s URL to `parsed_url`
        // 5. Set `url`’s query object to a new URLSearchParams object
        // 6. Initialize `url`’s query object with `query`
        let url = Self {
            url: parsed_url,
            search_params: JsNativeObject::new::<UrlSearchParamsClass>(
                UrlSearchParams::new(query),
                context,
            )?,
        };

        Ok(url)
    }

    /// Returns a boolean indicating whether or not a URL defined from a
    /// URL string and optional base URL string is parsable and valid.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-canparse
    pub fn can_parse(url: String, base: Option<String>) -> bool {
        // 1. Let `parsed_url` be the result of running the API URL parser on `url` with `base`, if given.
        let parsed_url = Self::parse_url(url, base);

        // 2. If `parsed_url` is failure, then return false
        // 3. Otherwise, return true
        parsed_url.is_some()
    }

    /// Returns a string containing the whole URL.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-href
    pub fn href(&self) -> String {
        self.url.to_string()
    }

    /// Updates the URL according the provided `href` value
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-href
    pub fn set_href(&mut self, href: &str) -> JsResult<()> {
        // 1. Let `parsed_url` be the result of running the basic URL parser on the given value.
        let parsed_url = InnerUrl::parse(href).map_err(|_| {
            // 2. If `parsed_url` is failure, then throw a TypeError.
            JsError::from_native(JsNativeError::typ().with_message("Failed to parse url"))
        })?;

        // 5. Let query be `parsed_url`'s query object list
        let query: Vec<_> = parsed_url.query_pairs().into_owned().collect();

        // 3. Set `self`'s URL to `parsed_url`.
        self.url = parsed_url;

        // 6. If `query` is non-null (not empty)
        if !query.is_empty() {
            // 4. Empty `self`'s query object's list
            // 6. (cont.) then set `self`’s query object’s list to `query`
            self.search_params.deref_mut().set_values(query)
        }

        Ok(())
    }

    /// Returns a string containing the origin of the URL, that is its scheme,
    /// its domain and its port
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-origin
    pub fn origin(&self) -> String {
        self.url.origin().ascii_serialization()
    }

    /// Returns a string containing the protocol scheme of the URL, including the final ':'.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://url.spec.whatwg.org/#dom-url-protocol
    pub fn protocol(&self) -> String {
        String::from(self.url.scheme())
    }

    pub fn set_protocol(&mut self, protocol: &str) -> JsResult<()> {
        self.url.set_scheme(protocol).map_err(|_| {
            JsError::from(JsNativeError::typ().with_message("Invalid protocol"))
        })
    }

    pub fn username(&self) -> String {
        String::from(self.url.username())
    }

    pub fn set_username(&mut self, username: &str) -> JsResult<()> {
        self.url.set_username(username).map_err(|_| {
            JsError::from_native(JsNativeError::typ().with_message("Invalid username"))
        })
    }

    pub fn password(&self) -> Option<String> {
        self.url.password().map(String::from)
    }

    pub fn set_password(&mut self, password: Option<&str>) -> JsResult<()> {
        self.url.set_password(password).map_err(|_| {
            JsError::from_native(JsNativeError::typ().with_message("Invalid password"))
        })
    }

    pub fn host(&self) -> Option<String> {
        self.url.host_str().map(|host| {
            if let Some(port) = self.url.port() {
                format!("{}:{}", host, port)
            } else {
                String::from(host)
            }
        })
    }

    pub fn set_host(&mut self, host: Option<&str>) -> JsResult<()> {
        fn invalid_host() -> JsError {
            JsError::from_native(JsNativeError::typ().with_message("Invalid host"))
        }

        fn invalid_port() -> JsError {
            JsError::from_native(JsNativeError::typ().with_message("Invalid port"))
        }

        if let Some(host) = host {
            // Split `host` into a `host` and a port, if given.
            let segments: Vec<&str> = host.split(':').collect();
            let (host, port) = match segments.len().cmp(&2) {
                Ordering::Greater => return Err(invalid_host()),
                Ordering::Less => (segments[0], None),
                Ordering::Equal => {
                    let port = segments[1].parse::<u16>().map_err(|_| invalid_port())?;

                    (segments[0], Some(port))
                }
            };

            // TODO: remove duplicated code
            self.url.set_host(Some(host)).map_err(|_| invalid_host())?;
            self.url.set_port(port).map_err(|_| invalid_port())?;
        } else {
            self.url.set_host(None).map_err(|_| invalid_host())?;
            self.url.set_port(None).map_err(|_| invalid_port())?;
        };

        Ok(())
    }

    pub fn hostname(&self) -> Option<String> {
        self.url.host_str().map(String::from)
    }

    pub fn set_hostname(&mut self, hostname: Option<&str>) -> JsResult<()> {
        self.url.set_host(hostname).map_err(|_| {
            JsError::from_native(JsNativeError::typ().with_message("Invalid hostname"))
        })
    }

    pub fn port(&self) -> Option<u16> {
        self.url.port_or_known_default()
    }

    pub fn set_port(&mut self, port: Option<u16>) -> JsResult<()> {
        self.url.set_port(port).map_err(|_| {
            JsError::from_native(JsNativeError::typ().with_message("Invalid port"))
        })
    }

    pub fn pathname(&self) -> String {
        String::from(self.url.path())
    }

    pub fn set_pathname(&mut self, path: &str) -> JsResult<()> {
        self.url.set_path(path);
        Ok(())
    }

    pub fn search(&self) -> Option<String> {
        self.url.query().map(String::from)
    }

    pub fn set_search(&mut self, search: Option<&str>) {
        self.url.set_query(search);
    }

    pub fn search_params(&self) -> JsObject {
        self.search_params.to_object()
    }

    pub fn hash(&self) -> Option<String> {
        self.url.fragment().map(String::from)
    }

    pub fn set_hash(&mut self, hash: Option<&str>) {
        self.url.set_fragment(hash);
    }
}

pub struct UrlClass;

impl Url {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Url`")
                    .into()
            })
    }
}

impl UrlClass {
    fn hash(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "hash",
            get:((url, context) => Ok(url.hash().into_js(context))),
            set:((url, hash: Option<String>, _context) => url.set_hash(hash.as_deref()))
        )
    }

    fn host(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "host",
            get:((url, context) => Ok(url.host().into_js(context))),
            set:((url, host: Option<String>, _context) => url.set_host(host.as_deref())?)
        )
    }

    fn hostname(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "hostname",
            get:((url, context) => Ok(url.hostname().into_js(context))),
            set:((url, hostname: Option<String>, _context) => url.set_hostname(hostname.as_deref())?)
        )
    }

    fn href(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "href",
            get:((url, context) => Ok(url.href().into_js(context))),
            set:((url, href: String, _context) => url.set_href(&href)?)
        )
    }

    fn origin(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "origin",
            get:((url, context) => Ok(url.origin().into_js(context)))
        )
    }

    fn password(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "href",
            get:((url, context) => Ok(url.password().into_js(context))),
            set:((url, password: Option<String>, _context) => url.set_password(password.as_deref())?)
        )
    }

    fn pathname(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "pathname",
            get:((url, context) => Ok(url.pathname().into_js(context))),
            set:((url, path: String, _context) => url.set_pathname(&path)?)
        )
    }

    fn port(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "port",
            get:((url, context) => Ok(url.port().into_js(context))),
            set:((url, port: Option<u16>, _context) => url.set_port(port)?)
        )
    }

    fn protocol(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "protocol",
            get:((url, context) => Ok(url.protocol().into_js(context))),
            set:((url, protocol: String, _context) => url.set_protocol(&protocol)?)
        )
    }

    fn search(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "search",
            get:((url, context) => Ok(url.search().into_js(context))),
            set:((url, search: Option<String>, _context) => url.set_search(search.as_deref()))
        )
    }

    fn search_params(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "searchParams",
            get:((url, _context) => Ok(url.search_params().into()))
        )
    }

    fn username(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Url,
            "username",
            get:((url, context) => Ok(url.username().into_js(context))),
            set:((url, search: String, context) => url.set_username(&search)?)
        )
    }

    fn can_parse(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let url: String = args.get_or_undefined(0).try_js_into(context)?;
        let base: Option<String> = args.get_or_undefined(1).try_js_into(context)?;

        Ok(Url::can_parse(url, base).into())
    }
}

impl NativeClass for UrlClass {
    type Instance = Url;

    const NAME: &'static str = "URL";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Url> {
        let url: String = args.get_or_undefined(0).try_js_into(context)?;
        let base: Option<String> = args.get_or_undefined(1).try_js_into(context)?;

        Url::new(url, base, context)
    }

    fn object_constructor(
        this: &JsNativeObject<Self::Instance>,
        _args: &[JsValue],
        _context: &mut Context<'_>,
    ) -> JsResult<()> {
        // 7. Set `this`’s query object’s URL object to `this`.
        this.deref_mut().search_params.deref_mut().set_url(this);

        Ok(())
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let hash = UrlClass::hash(class.context());
        let host = UrlClass::host(class.context());
        let hostname = UrlClass::hostname(class.context());
        let href = UrlClass::href(class.context());
        let origin = UrlClass::origin(class.context());
        let password = UrlClass::password(class.context());
        let pathname = UrlClass::pathname(class.context());
        let port = UrlClass::port(class.context());
        let protocol = UrlClass::protocol(class.context());
        let search = UrlClass::search(class.context());
        let search_params = UrlClass::search_params(class.context());
        let username = UrlClass::username(class.context());

        class
            .accessor(js_string!("hash"), hash, Attribute::all())
            .accessor(js_string!("host"), host, Attribute::all())
            .accessor(js_string!("hostname"), hostname, Attribute::all())
            .accessor(js_string!("href"), href, Attribute::all())
            .accessor(js_string!("origin"), origin, Attribute::all())
            .accessor(js_string!("password"), password, Attribute::all())
            .accessor(js_string!("pathname"), pathname, Attribute::all())
            .accessor(js_string!("port"), port, Attribute::all())
            .accessor(js_string!("protocol"), protocol, Attribute::all())
            .accessor(js_string!("search"), search, Attribute::all())
            .accessor(js_string!("searchParams"), search_params, Attribute::all())
            .accessor(js_string!("username"), username, Attribute::all())
            .static_method(
                js_string!("canParse"),
                1,
                NativeFunction::from_fn_ptr(UrlClass::can_parse),
            )
            .method(
                js_string!("toString"),
                0,
                NativeFunction::from_fn_ptr(UrlClass::to_string),
            )
            .method(
                js_string!("toJSON"),
                0,
                NativeFunction::from_fn_ptr(UrlClass::to_string),
            );

        Ok(())
    }
}

pub struct UrlApi;

impl jstz_core::Api for UrlApi {
    fn init(self, context: &mut Context<'_>) {
        UrlSearchParamsApi.init(context);
        register_global_class::<UrlClass>(context)
            .expect("The `URL` class shouldn't exist yet")
    }
}
