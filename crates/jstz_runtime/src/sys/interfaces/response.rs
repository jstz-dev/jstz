use deno_core::v8;

use crate::{js_class, js_constructor, js_getter, js_method, js_static_method};

use super::Headers;

js_class!(Response);

impl<'s> Response<'s> {
    js_constructor! { fn new() }

    js_getter! { fn status() -> u16 }

    js_getter! {
      #[js_name(statusText)]
      fn status_text() -> String
    }

    js_getter! { fn is_ok() -> bool }

    js_getter! { fn headers() -> Headers<'s> }

    js_getter! { fn body_used() -> bool }

    js_getter! { fn body() -> Option<v8::Local<'s, v8::Value>> }

    js_static_method! {
      #[js_name(ok)]
      fn new_ok() -> Self
    }

    js_static_method! {
      #[js_name(error)]
      fn new_error() -> Self
    }

    js_method! {
      #[js_name(arrayBuffer)]
      async fn array_buffer() -> deno_core::JsBuffer
    }
}
