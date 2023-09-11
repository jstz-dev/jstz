//! `jstz`'s implementation of JavaScript's `Response` Web API Class.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [WHATWG `Headers` specification][spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Response
//! [spec]: https://fetch.spec.whatwg.org/#response-class
use std::str::FromStr;

use boa_engine::{
    object::{builtins::JsPromise, Object},
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
};
use boa_gc::{custom_trace, Finalize, GcRefMut, Trace};
use http::{Response as InnerResponse, StatusCode};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};
use url::Url;

use super::{
    body::{Body, BodyWithType},
    header::{Headers, HeadersClass},
};

pub struct Response {
    response: InnerResponse<Body>,
    headers: JsNativeObject<Headers>,
    url: Option<Url>,
}

impl Finalize for Response {
    fn finalize(&self) {
        self.headers.finalize();
        self.response.body().finalize()
    }
}

unsafe impl Trace for Response {
    custom_trace!(this, {
        mark(&this.headers);
        mark(this.response.body());
    });
}

pub struct ResponseOptions {
    status: u16,
    headers: Headers,
}

impl Default for ResponseOptions {
    fn default() -> Self {
        Self {
            status: 200,
            headers: Default::default(),
        }
    }
}

impl Response {
    /// Creates a new Response object.
    ///
    /// More information:
    ///  - [WHATWG specification 1][spec1]
    ///  - [WHATWG specification 2][spec2]
    ///
    /// [spec1] https://fetch.spec.whatwg.org/#response-create
    /// [spec2] https://fetch.spec.whatwg.org/#initialize-a-response
    pub fn new(
        body_with_type: BodyWithType,
        options: ResponseOptions,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        // 1. If `init["status"]` is not in the range 200 to 500 inclusive,
        // 2. (FIXME:) SKIPPED
        // 3. Set response's response's status to `init["status"]`
        let status = StatusCode::from_u16(options.status).map_err(|_| {
            // 1. (cont.) then throw a RangeError
            JsError::from_native(
                JsNativeError::range().with_message("Invalid status code"),
            )
        })?;

        // 3. (FIXME:) SKIPPED

        // 5. If  `init["headers"]` exists, then fill response's headers with `init["headers"]`
        let mut headers = options.headers;

        // 6. If `body` was given, then:
        //    Default is `Body::null()`.
        let body = {
            // 1. (FIXME:) SKIPPED

            // 3. If `body_with_type`'s type is non-null and response's header list does
            //    not contain `Content-Type`
            if let Some(content_type) = body_with_type.content_type {
                if !headers.contains("Content-Type")? {
                    // 3. (cont.) then append `("Content-Type", content_type)` to response's
                    //    header list
                    headers.append("Content-Type", content_type)?;
                }
            };

            // 2. Set response's body to `body_with_type`'s body.
            body_with_type.body
        };

        let response =
            InnerResponse::builder()
                .status(status)
                .body(body)
                .map_err(|_| {
                    JsError::from_native(
                        JsNativeError::typ().with_message("Malformed response"),
                    )
                })?;

        Ok(Self {
            response,
            headers: JsNativeObject::new::<HeadersClass>(headers, context)?,
            url: None,
        })
    }

    /// Returns the Headers object associated with the response.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-headers
    pub fn headers(&self) -> &JsNativeObject<Headers> {
        &self.headers
    }

    /// Returns a boolean indicating whether the response was successful
    /// (status in the range 200 – 299) or not.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-ok
    pub fn ok(&self) -> bool {
        self.response.status().is_success()
    }

    /// Returns the status code of the response. (This will be 200 for a success).
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-status
    pub fn status(&self) -> u16 {
        self.response.status().as_u16()
    }

    /// Returns the status message corresponding to the status code. (e.g., OK for 200).
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-statustext
    pub fn status_text(&self) -> Option<String> {
        self.response.status().canonical_reason().map(String::from)
    }

    /// Returns whether or not the response is the result of a redirect
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-redirected
    pub fn redirected(&self) -> bool {
        self.url.is_some()
    }

    /// Returns the URL of the response.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-url
    pub fn url(&self) -> Option<&Url> {
        // self.url.as_deref() but Url doesn't implement deref :(
        match &self.url {
            Some(url) => Some(url),
            None => None,
        }
    }

    // FIXME: Missing `clone` and `type`
}

// Body mixin
impl Response {
    /// Return a boolean value that declares whether the body has been
    /// used in a response yet.
    pub fn body_used(&self) -> bool {
        self.response.body().is_used()
    }

    /// Returns a promise that resolves with an ArrayBuffer representation of the response body.
    pub fn array_buffer(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.response.body_mut().array_buffer(context)
    }

    /// Returns a promise that resolves with the result of parsing the response body text as JSON.
    pub fn json(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.response.body_mut().json(context)
    }

    /// Returns a promise that resolves with a text representation of the response body.
    pub fn text(&mut self, context: &mut Context<'_>) -> JsResult<JsPromise> {
        self.response.body_mut().text(context)
    }
}

pub struct ResponseBuilder;

impl ResponseBuilder {
    /// Returns a new Response object associated with a network error.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-error
    pub fn error(context: &mut Context<'_>) -> JsResult<Response> {
        // 1. Create a `Response` object given a new "network error".

        // Note: A network error is a response whose type is "error",
        //       status is 0, status message is the empty byte sequence,
        //       header list is « », body is null, and body info is a
        //       new response body info
        //
        // FIXME: jstz only supports status, body and headers here.
        // FIXME: http::Response won't support 0 as a status code
        let status = 500;
        let body = Body::null();
        let headers = Headers::new();

        Ok(Response {
            response: InnerResponse::builder().status(status).body(body).unwrap(),
            headers: JsNativeObject::new::<HeadersClass>(headers, context)?,
            url: None,
        })
    }

    /// Returns a new response with a different URL.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-redirect
    pub fn redirect(
        url: String,
        status: Option<u16>,
        context: &mut Context<'_>,
    ) -> JsResult<Response> {
        // 1. Let `parsed_url` be the result of parsing `url`
        let parsed_url = Url::from_str(&url).map_err(|_| {
            // 2. If `parsed_url` is failure, then throw a TypeError
            JsError::from_native(JsNativeError::typ().with_message("Invalid URL"))
        })?;

        // 2. If status is not a redirect status, then row a RangeError
        let status = StatusCode::from_u16(status.unwrap_or(302)).map_err(|_| {
            JsError::from_native(
                JsNativeError::range().with_message("Invalid status code"),
            )
        })?;
        if !status.is_redirection() {
            return Err(JsError::from_native(
                JsNativeError::range().with_message("Expected a redirect status"),
            ));
        };

        let mut headers = Headers::new();

        // 6. Let `location` be `parsed_url`, serialized and isomorphic encoded
        let location = parsed_url.to_string();

        // 7. Append `("Location", value)` to response's header list
        headers.append("Location", &location)?;

        // 4. Let `response` be the result of creating a Response object,
        //    given a new response, "immutable", and the current realm
        // 5. Set `response`'s response’s status to status
        // 8. Return `response`
        Ok(Response {
            response: InnerResponse::builder()
                .status(status)
                .body(Body::null())
                .unwrap(),
            headers: JsNativeObject::new::<HeadersClass>(headers, context)?,
            url: Some(parsed_url),
        })
    }

    /// Returns a new Response object for returning the provided JSON encoded data.
    ///
    /// More information:
    ///  - [WHATWG specification][spec]
    ///
    /// [spec] https://fetch.spec.whatwg.org/#dom-response-json
    pub fn json(value: &JsValue, context: &mut Context<'_>) -> JsResult<Response> {
        // 1, 2, 4. See `BodyWithType::json`
        let body = BodyWithType::json(value, context)?;

        // 3. Let `response` be the result of creating a `Response` object, given a
        //    new response, "response", and the current realm.
        // 4. Perform initialize a response given `response`, `init`, and
        //    `(body, "application/json")`.
        Response::new(body, Default::default(), context)
    }
}

pub struct ResponseClass;

impl Response {
    fn try_from_js<'a>(value: &'a JsValue) -> JsResult<GcRefMut<'a, Object, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Response`")
                    .into()
            })
    }
}

impl ResponseClass {
    fn static_error(
        _this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        Ok(
            JsNativeObject::new::<Self>(ResponseBuilder::error(context)?, context)?
                .inner()
                .clone(),
        )
    }

    fn static_json(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let value = args.get_or_undefined(0);

        Ok(
            JsNativeObject::new::<Self>(ResponseBuilder::json(value, context)?, context)?
                .inner()
                .clone(),
        )
    }

    fn static_redirect(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let url: String = args.get_or_undefined(0).try_js_into(context)?;
        let status: Option<u16> = args.get_or_undefined(1).try_js_into(context)?;

        Ok(JsNativeObject::new::<Self>(
            ResponseBuilder::redirect(url, status, context)?,
            context,
        )?
        .inner()
        .clone())
    }

    fn headers(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "headers",
            get:((response, _context) => Ok(response.headers().inner().clone()))
        )
    }

    fn ok(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "ok",
            get:((response, _context) => Ok(response.ok().into()))
        )
    }

    fn redirected(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "redirected",
            get:((response, _context) => Ok(response.redirected().into()))
        )
    }

    fn status(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "redirected",
            get:((response, _context) => Ok(response.status().into()))
        )
    }

    fn status_text(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "statusText",
            get:((response, context) => Ok(response.status_text().into_js(context)))
        )
    }

    fn url(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "url",
            get:((response, _context) => {
                match response.url() {
                    None => Ok(JsValue::null()),
                    Some(url) => Ok(url.to_string().into()),
                }
            })
        )
    }

    fn body_used(context: &mut Context<'_>) -> Accessor {
        accessor!(
            context,
            Response,
            "bodyUsed",
            get:((response, _context) => Ok(response.body_used().into()))
        )
    }

    fn array_buffer(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Response::try_from_js(this)?;

        Ok(request.array_buffer(context)?.into())
    }

    fn text(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Response::try_from_js(this)?;

        Ok(request.text(context)?.into())
    }

    fn json(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let mut request = Response::try_from_js(this)?;

        Ok(request.json(context)?.into())
    }
}

impl TryFromJs for ResponseOptions {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected `JsObject`"))
        })?;

        let status: u16 = if obj.has_property("status", context)? {
            obj.get("status", context)?.try_js_into(context)?
        } else {
            200
        };

        let headers: Headers = if obj.has_property("headers", context)? {
            obj.get("headers", context)?.try_js_into(context)?
        } else {
            Default::default()
        };

        Ok(Self { status, headers })
    }
}

impl NativeClass for ResponseClass {
    type Instance = Response;

    const NAME: &'static str = "Response";

    fn constructor(
        _this: &JsNativeObject<Self::Instance>,
        args: &[boa_engine::JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Self::Instance> {
        let body: BodyWithType = match args.get(0) {
            Some(value) => value.try_js_into(context)?,
            None => Default::default(),
        };

        let options: ResponseOptions = match args.get(1) {
            Some(options) => options.try_js_into(context)?,
            None => Default::default(),
        };

        Response::new(body, options, context)
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        let url = Self::url(class.context());
        let redirected = Self::redirected(class.context());
        let status = Self::status(class.context());
        let ok = Self::ok(class.context());
        let status_text = Self::status_text(class.context());
        let headers = Self::headers(class.context());
        let body_used = Self::body_used(class.context());

        class
            .static_method("error", 0, NativeFunction::from_fn_ptr(Self::static_error))
            .static_method(
                "redirect",
                1,
                NativeFunction::from_fn_ptr(Self::static_redirect),
            )
            .static_method("json", 1, NativeFunction::from_fn_ptr(Self::static_json))
            .accessor("url", url, Attribute::all())
            .accessor("redirected", redirected, Attribute::all())
            .accessor("status", status, Attribute::all())
            .accessor("ok", ok, Attribute::all())
            .accessor("statusText", status_text, Attribute::all())
            .accessor("headers", headers, Attribute::all())
            .accessor("bodyUsed", body_used, Attribute::all())
            .method(
                "arrayBuffer",
                0,
                NativeFunction::from_fn_ptr(Self::array_buffer),
            )
            .method("text", 0, NativeFunction::from_fn_ptr(Self::text))
            .method("json", 0, NativeFunction::from_fn_ptr(Self::json));

        Ok(())
    }
}

pub struct ResponseApi;

impl jstz_core::Api for ResponseApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<ResponseClass>(context)
            .expect("The `Response` class shouldn't exist yet")
    }
}
