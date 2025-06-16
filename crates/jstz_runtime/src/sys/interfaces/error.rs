use crate::{js_class, js_getter};
use deno_core::v8;

js_class!(Error);

impl<'s> Error<'s> {
    js_getter! { fn name() -> String }

    js_getter! { fn message() -> String }
}
