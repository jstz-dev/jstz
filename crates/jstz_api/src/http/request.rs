//! `jstz`'s implementation of JavaScript's `Request` Web API Class.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `Headers` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request
//! [spec]: https://fetch.spec.whatwg.org/#request-class
use std::str::FromStr;

use boa_engine::{
    js_string,
    object::{builtins::JsPromise, Object},
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{custom_trace, Finalize, GcRefMut, Trace};
use http::{Method, Request as InnerRequest, Uri};

use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};
use url::Url;

use super::{
    body::{Body, BodyWithType, HttpBody},
    header::{Headers, HeadersClass},
};

pub enum RequestInfo {
    Request(Request),
    String(String),
}

#[derive(Default)]
pub struct RequestOptions {
    method: Method,
    headers: Option<Headers>,
    body: BodyWithType,
}

pub struct Request {
    request: InnerRequest<Body>,
    headers: JsNativeObject<Headers>,
    url: Url,
}

impl Request {
    pub fn from_http_request(
        request: http::Request<HttpBody>,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        let url = Url::from_str(&request.uri().to_string()).map_err(|_| {
            JsError::from_native(JsNativeError::typ().with_message("Expected valid URL"))
        })?;
        let headers = JsNativeObject::new::<HeadersClass>(
            Headers::from_http_headers(request.headers().clone(), context)?,
            context,
        )?;

        let request = {
            let (parts, body) = request.into_parts();
            let body = Body::from_http_body(body, context).map_err(|_| {
                JsError::from_native(
                    JsNativeError::typ().with_message("Expected valid body"),
                )
            })?;
            http::Request::from_parts(parts, body)
        };
        Ok(Self {
            request,
            headers,
            url,
        })
    }
}

fn clone_inner_request<T: Clone>(request: &InnerRequest<T>) -> InnerRequest<T> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body = request.body().clone();

    let mut request = http::Request::builder().method(method).uri(uri);
    if let Some(h) = request.headers_mut() {
        *h = headers;
    }

    request
        .body(body)
        .expect("Cannot construct a malformed request from a valid one")
}

impl Clone for Request {
    fn clone(&self) -> Self {
        Self {
            request: clone_inner_request(&self.request),
            headers: self.headers.clone(),
            url: self.url.clone(),
        }
    }
}

impl Finalize for Request {
    fn finalize(&self) {
        self.headers.finalize();
        self.request.body().finalize()
    }
}

unsafe impl Trace for Request {
    custom_trace!(this, {
        mark(&this.headers);
        mark(this.request.body());
    });
}

impl Request {
    fn check_method_with_body(method: &Method, has_body: bool) -> JsResult<()> {
        match (has_body, method) {
            (
                true,
                &Method::GET
                | &Method::HEAD
                | &Method::CONNECT
                | &Method::OPTIONS
                | &Method::TRACE,
            ) => Err(JsError::from_native(JsNativeError::typ().with_message(
                format!("{} cannot have a body.", method.as_str()),
            ))),
            (false, &Method::POST | &Method::PUT | &Method::PATCH) => {
                Err(JsError::from_native(JsNativeError::typ().with_message(
                    format!("{} must have a body", method.as_str()),
                )))
            }
            _ => Ok(()),
        }
    }

    fn check_url_scheme(url: &Url) -> JsResult<()> {
        if url.scheme() != "tezos" {
            return Err(JsError::from_native(
                JsNativeError::typ().with_message("Invalid scheme"),
            ));
        }

        Ok(())
    }

    /// [spec] https://fetch.spec.whatwg.org/#request-create
    pub fn new(
        info: RequestInfo,
        options: RequestOptions,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        // 1. Let `request` be null
        // 3. Let `base_url` be `this's` relevant settings object's API base URL
        //    (This is managed by the `Url` library)
        let mut request = match info {
            // 5. If `info` is a string, then:
            RequestInfo::String(url) => {
                // 1. Let `parsed_url` be the result of parsing `url` with `base_url`
                let parsed_url = Url::from_str(&url).map_err(|_| {
                    // 2. If `parsed_url` is failure, then throw a TypeError
                    JsError::from_native(JsNativeError::typ().with_message("Invalid URL"))
                })?;

                // 3. If `parsed_url` includes credentials, then throw a TypeError
                // FIXME: SKIPPED
                // if parsed_url.has_authority() {
                //     return Err(JsError::from_native(
                //         JsNativeError::typ()
                //             .with_message("URL cannot contain credentials"),
                //     ));
                // }

                // 4. Set `request` to a new request whose URL is `parsed_url`
                let request = InnerRequest::builder()
                    .uri(Uri::from_str(&url).map_err(|_| {
                        JsError::from_native(
                            JsNativeError::typ().with_message("Invalid URI"),
                        )
                    })?)
                    .body(Body::null())
                    .unwrap();

                Request {
                    request,
                    url: parsed_url,
                    headers: JsNativeObject::new::<HeadersClass>(
                        Headers::default(),
                        context,
                    )?,
                }
            }
            // 6. Otheriwse:
            RequestInfo::Request(request) => {
                // 1. Assert: input is a `Request` object
                // 2. Set reqiest to input's request
                request.clone()
            }
        };

        // TEZOS SPECIFIC: Check if URL scheme is "tezos"
        Request::check_url_scheme(&request.url)?;

        // 7-24. (FIXME:) SKIPPED

        // 25. If init["method"] exists, then:
        // 1. Let `method` be `init["method"]`
        let method = options.method;

        // 2. If `method` is not a method or `method` is a forbidden method,
        //    then throw a TypeError
        // 3. Normalize method
        //
        // Already done since `method` is a `Method`, not a `String`

        // 4. Set `request`'s method to `method`
        *request.request.method_mut() = method;

        // 26-32: (FIXME:) SKIPPED

        // 33. If init is not empty, then
        // Note: init (aka options) has default values

        // 1. Let `headers` be a copy of `this`'s headers and its associated header list.
        // 2. If `init["headers"]` exists, then set `headers` to `init["headers"]`.
        // 3. Empty `this`’s headers’s header list.
        // 4. If `headers` is a Headers object, then
        //    for each `header` of its header list, append header to this’s headers.
        // 5. Otherwise, fill this’s headers with headers.
        //
        // Note: This is equivalent to the `into_headers()` function
        //       and setting the headers object of `request`.
        if let Some(headers) = options.headers {
            request.headers = JsNativeObject::new::<HeadersClass>(headers, context)?;
        }

        // 34.
        // 35. If either `init["body"]` exists and is non-null or `input_body` is non-null, and
        //     request's method is `GET` or `HEAD`, then throw a TypeError
        let body_with_type = options.body;
        Request::check_method_with_body(
            request.method(),
            !body_with_type.body.is_null(),
        )?;

        // 37. If `init["body"]` exists and is non-null, then:
        // 1. Let `body_with_type` be the result of extracting `init["body"]`, with keepalive
        //    set to request's keepalive.

        // 2. Set `request`'s body with `body_with_type`'s body
        *request.request.body_mut() = body_with_type.body;

        // 3. Let `content_type` be `body_with_type`' s type
        let content_type = body_with_type.content_type;

        // 4. If `context_type` is non-null and `request`'s header's does not contain `Content-Type`,
        //    then append `("Content-Type", content_type)` to `request`'s headers
        if let Some(content_type) = content_type {
            if !request.headers.deref().contains_key("Content-Type") {
                request
                    .headers
                    .deref_mut()
                    .append("Content-Type", content_type)?
            }
        }

        // 38-42: (FIXME:) SKIPPED

        Ok(request)
    }

    pub fn method(&self) -> &Method {
        self.request.method()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn headers(&self) -> &JsNativeObject<Headers> {
        &self.headers
    }

    pub fn array_buffer(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.request.body_mut().array_buffer(context)
    }

    pub fn json(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.request.body_mut().json(context)
    }

    pub fn text(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.request.body_mut().text(context)
    }

    pub fn body_used(&self) -> bool {
        self.request.body().is_used()
    }
}

pub struct RequestClass;

impl Request {
    fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Request`")
                    .into()
            })
    }
}

impl RequestClass {
    fn method(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Request,
            "method",
            get:((request, context) => Ok(request.method().to_string().into_js(context)))
        )
    }

    fn url(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Request,
            "url",
            get:((request, context) => Ok(request.url().to_string().into_js(context)))
        )
    }

    fn headers(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Request,
            "headers",
            get:((request, _context) => Ok(request.headers().inner().clone()))
        )
    }

    fn body_used(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Request,
            "bodyUsed",
            get:((request, _context) => Ok(request.body_used().into()))
        )
    }

    fn array_buffer(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Request::try_from_js(this)?;

        Ok(request.array_buffer(context)?.into())
    }

    fn text(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Request::try_from_js(this)?;

        Ok(request.text(context)?.into())
    }

    fn json(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Request::try_from_js(this)?;

        Ok(request.json(context)?.into())
    }
}

impl TryFromJs for RequestInfo {
    fn try_from_js(value: &JsValue, _context: &mut Context<'_>) -> JsResult<Self> {
        if let Some(string) = value.as_string() {
            Ok(Self::String(string.to_std_string_escaped()))
        } else {
            let request = Request::try_from_js(value)?;
            Ok(Self::Request(request.clone()))
        }
    }
}

fn method_try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Method> {
    let string: String = value.try_js_into(context)?;

    Method::from_str(&string).map_err(|_| {
        JsError::from_native(JsNativeError::typ().with_message("Invalid method"))
    })
}

impl TryFromJs for RequestOptions {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected object"))
        })?;

        let method: Method = if obj.has_property(js_string!("method"), context)? {
            method_try_from_js(&obj.get(js_string!("method"), context)?, context)?
        } else {
            Default::default()
        };

        let headers: Option<Headers> =
            if obj.has_property(js_string!("headers"), context)? {
                Some(Headers::from_init(
                    obj.get(js_string!("headers"), context)?
                        .try_js_into(context)?,
                )?)
            } else {
                Default::default()
            };

        let body: BodyWithType = if obj.has_property(js_string!("body"), context)? {
            obj.get(js_string!("body"), context)?.try_js_into(context)?
        } else {
            Default::default()
        };

        Ok(Self {
            method,
            headers,
            body,
        })
    }
}

impl NativeClass for RequestClass {
    type Instance = Request;

    const NAME: &'static str = "Request";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Self::Instance> {
        let info: RequestInfo = args.get_or_undefined(0).try_js_into(context)?;

        let options: RequestOptions = match args.get(1) {
            Some(value) => value.try_js_into(context)?,
            None => Default::default(),
        };

        Request::new(info, options, context)
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let body_used = Self::body_used(class.context());
        let headers = Self::headers(class.context());
        let method = Self::method(class.context());
        let url = Self::url(class.context());

        class
            .accessor(js_string!("bodyUsed"), body_used, Attribute::all())
            .accessor(js_string!("headers"), headers, Attribute::all())
            .accessor(js_string!("method"), method, Attribute::all())
            .accessor(js_string!("url"), url, Attribute::all())
            .method(
                js_string!("arrayBuffer"),
                0,
                NativeFunction::from_fn_ptr(Self::array_buffer),
            )
            .method(
                js_string!("json"),
                0,
                NativeFunction::from_fn_ptr(Self::json),
            )
            .method(
                js_string!("text"),
                0,
                NativeFunction::from_fn_ptr(Self::text),
            );

        Ok(())
    }
}

pub struct RequestApi;

impl jstz_core::Api for RequestApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<RequestClass>(context)
            .expect("The `Request` class shouldn't exist yet")
    }
}
