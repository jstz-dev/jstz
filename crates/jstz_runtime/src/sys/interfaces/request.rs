use deno_core::v8;

use super::Headers;
use crate::sys::js::convert::Serde;
use crate::{js_class, js_constructor, js_getter, js_setter};

js_class!(Request);

impl<'s> Request<'s> {
    js_constructor! { fn new_with_request(input: Request<'s>) }

    js_constructor! { fn new_with_string(input: String) }

    js_constructor! { fn new_with_request_and_init(input: Request<'s>, init: RequestInit<'s>) }

    js_constructor! { fn new_with_string_and_init(input: String, init: RequestInit<'s>) }

    js_getter! { fn method() -> String }

    js_getter! { fn url() -> String }

    js_getter! { fn headers() -> Headers<'s> }

    js_getter! { fn body_used() -> bool }

    js_getter! { async fn body() -> Option<v8::Local<'s, v8::Value>> }
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
        fn set_method(value: Serde<deno_core::ByteString>)
    }
}
