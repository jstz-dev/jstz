use deno_core::{v8::{self}, ByteString};

use crate::{
    js_class, js_constructor, js_entries, js_keys, js_method, js_values, sys::js::convert::Serde
};

js_class!(Headers);

impl<'s> Headers<'s> {
    js_constructor! { fn new() }

    js_constructor! { fn new_with_headers(init: Headers<'s>) }

    js_constructor! { fn new_with_sequence(init: Serde<Vec<(String, String)>>) }

    js_constructor! { fn new_with_sequence_v8(init: Serde<Vec<(ByteString, ByteString)>>) }

    js_method! { fn append(name: String, value: String) }

    js_method! { fn delete(name: String) }

    js_method! { fn get(name: String) -> String }

    js_method! { fn has(name: String) -> bool }

    js_method! { fn set(name: String, value: String) }

    js_method! {
        #[js_name(forEach)]
        fn for_each(callback: v8::Local<'s, v8::Function>)
    }

    js_entries!(ByteString);

    js_keys!();

    js_values!(ByteString);

    // TODO():
    // Support `new_with_record` -- requires a record-like init.
}
