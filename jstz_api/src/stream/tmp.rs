use std::{marker::PhantomData, ops::Deref};

use boa_engine::{
    js_string, object::builtins::JsFunction, value::TryFromJs, Context, JsArgs, JsResult,
    JsValue, NativeFunction,
};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};

use boa_gc::{custom_trace, Finalize, Trace};

use super::abstractions::underlying_source::UnderlyingSource;

#[macro_export]
macro_rules! todo_boa_type {
    ( $t:ident ) => {
        #[derive(Debug)]
        pub struct $t;

        impl Finalize for $t {
            fn finalize(&self) {
                todo!()
            }
        }

        unsafe impl Trace for $t {
            custom_trace!(this, { todo!() });
        }

        impl IntoJs for $t {
            fn into_js(self, _context: &mut Context<'_>) -> JsValue {
                todo!()
            }
        }

        impl TryFromJs for $t {
            fn try_from_js(
                _value: &JsValue,
                _context: &mut Context<'_>,
            ) -> JsResult<Self> {
                todo!()
            }
        }
    };
}

todo_boa_type!(Todo);

todo_boa_type!(ReadableStreamDefaultController);

todo_boa_type!(ReadableByteStreamController);
