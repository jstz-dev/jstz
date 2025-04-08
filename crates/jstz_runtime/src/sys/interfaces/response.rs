use deno_core::v8;

use crate::{
    js_class, js_constructor, js_getter, js_method, js_setter, js_static_method,
};

use super::Headers;

js_class!(Response);

impl<'s> Response<'s> {
    js_constructor! { fn new() }

    js_constructor! { fn new_with_body(body: v8::Local<'s, v8::Value>) }

    js_constructor! { fn new_with_body_and_init(body: v8::Local<'s, v8::Value>, init: ResponseInit<'s>) }

    js_static_method! {
      #[js_name(json)]
      fn new_json(value: v8::Local<'s, v8::Value>) -> Self
    }

    js_static_method! {
      #[js_name(json)]
      fn new_json_with_init(value: v8::Local<'s, v8::Value>, init: ResponseInit<'s>) -> Self
    }

    js_static_method! {
      #[js_name(redirect)]
      fn new_redirect(url: String) -> Self
    }

    js_static_method! {
      #[js_name(redirect)]
      fn new_redirect_with_status(url: String, status: u16) -> Self
    }

    js_static_method! {
      #[js_name(error)]
      fn new_error() -> Self
    }

    js_getter! { fn status() -> u16 }

    js_getter! {
      #[js_name(statusText)]
      fn status_text() -> String
    }

    js_getter! {
      fn url() -> String
    }

    js_getter! {
      #[js_name(type)]
      fn r#type() -> String
    }

    js_getter! {
      #[js_name(ok)]
      fn is_ok() -> bool
    }

    js_getter! { fn headers() -> Headers<'s> }

    js_getter! {
      #[js_name(redirected)]
      fn is_redirected() -> bool
    }

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

js_class!(ResponseInit);

impl<'s> ResponseInit<'s> {
    pub fn new(scope: &mut v8::HandleScope<'s>) -> Self {
        Self(v8::Object::new(scope))
    }

    js_setter! {
        #[js_name(status)]
        fn set_status(value: u16)
    }

    js_setter! {
        #[js_name(statusText)]
        fn set_status_text(value: String)
    }

    js_setter! {
        #[js_name(headers)]
        fn set_headers(value: Headers<'s>)
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

        let response = Response::new(scope).unwrap();
        assert_eq!(response.status(scope).unwrap(), 200);
    }

    #[test]
    fn test_new_with_body() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let body = v8::String::new(scope, "Hello World").unwrap();
        let response = Response::new_with_body(scope, body.into()).unwrap();
        assert!(!response.is_body_used(scope).unwrap());
    }

    #[test]
    fn test_new_with_body_and_init() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let body = v8::String::new(scope, "Hello World").unwrap();
        let init = ResponseInit::new(scope);
        init.set_status(scope, 200).unwrap();
        init.set_status_text(scope, "SuperSmashingGreat".into())
            .unwrap();

        let response =
            Response::new_with_body_and_init(scope, body.into(), init).unwrap();
        assert_eq!(response.status(scope).unwrap(), 200);
        assert_eq!(response.status_text(scope).unwrap(), "SuperSmashingGreat");
    }

    #[tokio::test]
    async fn test_new_json() {
        init_test_setup! { runtime = runtime; };

        let array_buffer = {
            let scope = &mut runtime.handle_scope();

            let value = v8::String::new(scope, "Hello World").unwrap();
            let response = Response::new_json(scope, value.into()).unwrap();
            assert_eq!(response.status(scope).unwrap(), 200);
            assert_eq!(response.r#type(scope).unwrap(), "default");
            assert!(response.is_ok(scope).unwrap());
            assert!(!response.is_redirected(scope).unwrap());
            assert!(!response.is_body_used(scope).unwrap());

            response.array_buffer(scope).unwrap()
        };

        let array_buffer = array_buffer.with_runtime(&mut runtime).await.unwrap();
        let buffer = array_buffer.as_ref();

        assert_eq!(buffer.len(), 13);
        assert_eq!(buffer, b"\"Hello World\"");
    }

    #[test]
    fn test_new_error() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let response = Response::new_error(scope).unwrap();
        assert_eq!(response.r#type(scope).unwrap(), "error");
    }

    #[test]
    fn test_new_redirect() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let response =
            Response::new_redirect(scope, "https://example.com/".into()).unwrap();
        assert_eq!(response.status(scope).unwrap(), 302);

        let headers = response.headers(scope).unwrap();
        assert_eq!(
            headers.get(scope, "Location".into()).unwrap().unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_body() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let body = v8::String::new(scope, "Hello World").unwrap();
        let response = Response::new_with_body(scope, body.into()).unwrap();
        let body = response.body(scope).unwrap();
        assert!(body.is_some());
        let body = body.unwrap();
        assert!(!body.is_promise())
    }

    #[test]
    fn test_headers() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        let init = ResponseInit::new(scope);
        init.set_headers(scope, headers).unwrap();
        let body = v8::null(scope);
        let response =
            Response::new_with_body_and_init(scope, body.into(), init).unwrap();

        let headers = response.headers(scope).unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn test_array_buffer() {
        init_test_setup! { runtime = runtime ;};

        let (global_response, array_buffer) = {
            let scope = &mut runtime.handle_scope();
            let body = v8::String::new(scope, "Hello World").unwrap();
            let response = Response::new_with_body(scope, body.into()).unwrap();
            let response_value = response.clone().to_v8(scope).unwrap();
            let global_response = v8::Global::new(scope, response_value);
            (global_response, response.array_buffer(scope).unwrap())
        };

        let array_buffer = array_buffer.with_runtime(&mut runtime).await.unwrap();
        let buffer = array_buffer.as_ref();

        assert_eq!(buffer, b"Hello World");
        assert_eq!(buffer.len(), 11);

        let scope = &mut runtime.handle_scope();
        let response_value = v8::Local::new(scope, global_response);
        let response = Response::from_v8(scope, response_value).unwrap();
        assert!(response.is_body_used(scope).unwrap());
    }
}
