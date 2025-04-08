use deno_core::{v8, ByteString};

use super::Headers;
use crate::sys::js::convert::Serde;
use crate::{js_class, js_constructor, js_getter, js_method, js_setter};

js_class!(Request);

impl<'s> Request<'s> {
    js_constructor! { fn new_with_request(input: Request<'s>) }

    js_constructor! { fn new_with_string(input: String) }

    js_constructor! { fn new_with_request_and_init(input: Request<'s>, init: RequestInit<'s>) }

    js_constructor! { fn new_with_string_and_init(input: String, init: RequestInit<'s>) }

    js_getter! { fn method() -> String }

    js_getter! { fn url() -> String }

    js_getter! { fn headers() -> Headers<'s> }

    js_getter! {
        #[js_name(bodyUsed)]
        fn is_body_used() -> bool
    }

    js_getter! { fn body() -> Option<v8::Local<'s, v8::Value>> }

    js_method! {
        #[js_name(arrayBuffer)]
        async fn array_buffer() -> deno_core::JsBuffer
    }
}

js_class!(RequestInit);

impl<'s> RequestInit<'s> {
    pub fn new(scope: &mut v8::HandleScope<'s>) -> Self {
        Self(v8::Object::new(scope))
    }

    js_setter! {
        #[js_name(body)]
        fn set_body(value: v8::Local<'s, v8::Value>)
    }

    js_setter! {
        #[js_name(headers)]
        fn set_headers(headers: Headers<'s>)
    }

    js_setter! {
        #[js_name(method)]
        fn set_method(value: Serde<ByteString>)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        init_test_setup,
        sys::js::convert::{FromV8, ToV8},
    };

    use super::*;

    #[test]
    fn test_new() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let request =
            Request::new_with_string(scope, "https://example.com".into()).unwrap();

        assert_eq!(request.url(scope).unwrap(), "https://example.com/");
        assert_eq!(request.method(scope).unwrap(), "GET");
    }

    #[test]
    fn test_new_with_request() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let request =
            Request::new_with_string(scope, "https://example.com".into()).unwrap();
        let new_request = Request::new_with_request(scope, request.clone()).unwrap();

        assert_eq!(new_request.url(scope).unwrap(), "https://example.com/");
        assert_eq!(new_request.method(scope).unwrap(), "GET");

        assert_ne!(request, new_request);
    }

    #[test]
    fn test_new_with_request_and_init() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let request =
            Request::new_with_string(scope, "https://example.com".into()).unwrap();
        let init = RequestInit::new(scope);
        let body = v8::null(scope);
        let headers = Headers::new(scope).unwrap();
        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        init.set_method(scope, ByteString::from("POST").into())
            .unwrap();
        init.set_body(scope, body.into()).unwrap();
        init.set_headers(scope, headers).unwrap();

        let new_request =
            Request::new_with_request_and_init(scope, request.clone(), init).unwrap();

        assert_eq!(new_request.url(scope).unwrap(), "https://example.com/");
        assert_eq!(new_request.method(scope).unwrap(), "POST");
        assert_eq!(
            new_request
                .headers(scope)
                .unwrap()
                .get(scope, "Content-Type".into())
                .unwrap()
                .unwrap(),
            "application/json"
        );
        assert_eq!(new_request.body(scope).unwrap(), None);

        assert_ne!(request, new_request);
    }

    #[test]
    fn test_new_with_string_and_init() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let init = RequestInit::new(scope);
        let body = v8::null(scope);
        let headers = Headers::new(scope).unwrap();
        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        init.set_method(scope, ByteString::from("POST").into())
            .unwrap();
        init.set_body(scope, body.into()).unwrap();
        init.set_headers(scope, headers).unwrap();

        let new_request =
            Request::new_with_string_and_init(scope, "https://example.com".into(), init)
                .unwrap();

        assert_eq!(new_request.url(scope).unwrap(), "https://example.com/");
        assert_eq!(new_request.method(scope).unwrap(), "POST");
        assert_eq!(
            new_request
                .headers(scope)
                .unwrap()
                .get(scope, "Content-Type".into())
                .unwrap()
                .unwrap(),
            "application/json"
        );
        assert_eq!(new_request.body(scope).unwrap(), None);
    }

    #[test]
    fn test_method() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let request =
            Request::new_with_string(scope, "https://example.com".into()).unwrap();

        assert_eq!(request.method(scope).unwrap(), "GET");
    }

    #[test]
    fn test_url() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let request =
            Request::new_with_string(scope, "https://example.com".into()).unwrap();

        assert_eq!(request.url(scope).unwrap(), "https://example.com/");
    }

    #[test]
    fn test_headers() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();
        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();
        let init = RequestInit::new(scope);
        init.set_headers(scope, headers).unwrap();
        let request =
            Request::new_with_string_and_init(scope, "https://example.com".into(), init)
                .unwrap();

        let headers = request.headers(scope).unwrap();
        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );

        headers
            .append(scope, "Accept".into(), "application/json".into())
            .unwrap();
        assert_eq!(
            headers.get(scope, "Accept".into()).unwrap().unwrap(),
            "application/json"
        );

        assert_eq!(
            request
                .headers(scope)
                .unwrap()
                .get(scope, "Accept".into())
                .unwrap()
                .unwrap(),
            "application/json"
        );
        assert_eq!(
            request
                .headers(scope)
                .unwrap()
                .get(scope, "Content-Type".into())
                .unwrap()
                .unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_body() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let init = RequestInit::new(scope);
        let body = v8::String::new(scope, "Hello World").unwrap();
        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "text/plain".into())
            .unwrap();
        init.set_method(scope, ByteString::from("POST").into())
            .unwrap();
        init.set_body(scope, body.into()).unwrap();
        init.set_headers(scope, headers).unwrap();

        let request =
            Request::new_with_string_and_init(scope, "https://example.com".into(), init)
                .unwrap();
        let body = request.body(scope).unwrap();

        assert!(body.is_some());
        assert!(!request.is_body_used(scope).unwrap());
        assert!(!body.unwrap().is_promise());
    }

    #[tokio::test]
    async fn test_array_buffer() {
        init_test_setup! { runtime = runtime; };

        let (global_request, array_buffer) = {
            let scope = &mut runtime.handle_scope();
            let init = RequestInit::new(scope);
            let body = v8::String::new(scope, "Hello World").unwrap();
            let headers = Headers::new(scope).unwrap();
            headers
                .append(scope, "Content-Type".into(), "text/plain".into())
                .unwrap();
            init.set_method(scope, ByteString::from("POST").into())
                .unwrap();
            init.set_body(scope, body.into()).unwrap();
            init.set_headers(scope, headers).unwrap();
            let request = Request::new_with_string_and_init(
                scope,
                "https://example.com".into(),
                init,
            )
            .unwrap();
            let request_value = request.clone().to_v8(scope).unwrap();
            let global_request = v8::Global::new(scope, request_value);
            (global_request, request.array_buffer(scope).unwrap())
        };

        let array_buffer = array_buffer.with_runtime(&mut runtime).await.unwrap();
        let buffer = array_buffer.as_ref();

        assert_eq!(buffer, b"Hello World");
        assert_eq!(buffer.len(), 11);

        let scope = &mut runtime.handle_scope();
        let request_value = v8::Local::new(scope, global_request);
        let request = Request::from_v8(scope, request_value).unwrap();
        assert!(request.is_body_used(scope).unwrap());
    }
}
