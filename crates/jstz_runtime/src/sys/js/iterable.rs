use std::marker::PhantomData;

use deno_core::ascii_str;
use deno_core::v8;

use super::class::{instance_call_method, instance_get, JsClass};
use super::convert::FromV8;
use crate::error::Result;

const VALUE_KEY: v8::OneByteConst =
    v8::String::create_external_onebyte_const("value".as_bytes());
const NEXT_KEY: v8::OneByteConst =
    v8::String::create_external_onebyte_const("next".as_bytes());

/// Javascript Iterable type
pub struct Iterable<'s, T: FromV8<'s>> {
    pub js_iterator: v8::Local<'s, v8::Object>,
    pub marker: PhantomData<T>,
}

impl<'s, T> Iterable<'s, T>
where
    T: FromV8<'s>,
{
    /// Calls `next` method on the Iterable. If underlying type is not Iterable,
    /// returns [`crate::error::RuntimeError`]
    pub fn next(&self, scope: &mut v8::HandleScope<'s>) -> Result<Option<T>> {
        let method_name = v8::String::new_from_onebyte_const(scope, &NEXT_KEY).unwrap();
        let result =
            instance_call_method::<Self>(scope, &self.js_iterator, method_name, &[])?;
        iter_item_from_v8(scope, result)
    }

    pub fn iter<'a>(
        &'a self,
        scope: &'a mut v8::HandleScope<'s>,
    ) -> impl Iterator<Item = T> + use<'a, 's, T> {
        ScopedIterable {
            scope,
            iterable: self,
        }
    }
}

fn iter_item_from_v8<'s, T: FromV8<'s>>(
    scope: &mut v8::HandleScope<'s>,
    item: v8::Local<'s, v8::Value>,
) -> Result<Option<T>> {
    let object = item.try_cast::<v8::Object>()?;
    let value_prop = v8::String::new_from_onebyte_const(scope, &VALUE_KEY).unwrap();
    let value = instance_get(scope, &object, value_prop)?;
    <Option<T> as FromV8>::from_v8(scope, value)
}

/// JS Iterable that holds a reference to the scope. Unlike [`Iterable`],
/// [`ScopedIterable`] implements [`Iterator`]
pub struct ScopedIterable<'a, 's, T: FromV8<'s>> {
    scope: &'a mut v8::HandleScope<'s>,
    iterable: &'a Iterable<'s, T>,
}

impl<'a, 's, T: FromV8<'s>> Iterator for ScopedIterable<'a, 's, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterable.next(self.scope).ok().flatten()
    }
}

impl<'s, T: FromV8<'s>> JsClass for Iterable<'s, T> {
    const JS_CLASS_NAME: deno_core::FastStaticString = ascii_str!("Iterable");
}

#[macro_export]
macro_rules! js_impl_iterable {
    ($kind:ident $return_type:ty) => {
        pub fn $kind(
            &self,
            scope: &mut v8::HandleScope<'s>,
        ) -> $crate::error::Result<$crate::sys::js::iterable::Iterable<'s, $return_type>>
        {
            let method_name = v8::String::new(scope, stringify!($kind)).unwrap();
            let js_iterator = $crate::sys::js::class::instance_call_method::<Self>(
                scope,
                &self.0,
                method_name,
                &[],
            )?
            .try_cast()
            .unwrap();
            Ok($crate::sys::js::iterable::Iterable {
                js_iterator,
                marker: std::marker::PhantomData,
            })
        }
    };
}

#[macro_export]
macro_rules! js_impl_known_iterable {
    (entries $return_type:ty) => {
        $crate::js_impl_iterable!(entries (deno_core::ByteString, $return_type));
    };
    (values $return_type:ty) => {
        $crate::js_impl_iterable!(values $return_type);
    };
    (keys) => {
        $crate::js_impl_iterable!(keys deno_core::ByteString);
    }
}

/// Generates `.entries()` on JS types that extend Iterable
#[macro_export]
macro_rules! js_entries {
    ($return_type:ty) => {
        $crate::js_impl_known_iterable!(entries $return_type);
    };
}

/// Generates `.values()` on JS types that extend Iterable
#[macro_export]
macro_rules! js_values {
    ($return_type:ty) => {
        $crate::js_impl_known_iterable!(values $return_type);
    };
}

/// Generates `.keys()` on JS types that extend Iterable
#[macro_export]
macro_rules! js_keys {
    () => {
        $crate::js_impl_known_iterable!(keys);
    };
}
