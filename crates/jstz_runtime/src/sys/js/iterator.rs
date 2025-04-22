use std::marker::PhantomData;

use deno_core::v8::{self, HandleScope};

use super::convert::{FromV8, ToV8};

use super::class::instance_call_method;

pub struct ScopedIterator<'s, T: FromV8<'s>> {
    pub js_iterator: v8::Local<'s, v8::Object>,
    pub marker: PhantomData<T>,
}

impl<'s, T> ScopedIterator<'s, T>
where
    T: FromV8<'s>,
{
    pub fn next(&mut self, scope: &mut v8::HandleScope<'s>) -> Option<T> {
        let method_name = v8::String::new(scope, "next").unwrap();
        let result = instance_call_method(scope, &self.js_iterator, method_name, &[]);
        iter_item_from_v8(scope, result)
    }
}

fn iter_item_from_v8<'s, T: FromV8<'s>>(scope: &mut HandleScope<'s>, item: v8::Local<'s, v8::Value>) -> Option<T> {
    let object = item.try_cast::<v8::Object>().unwrap();
    let value_key = "value".to_v8(scope);
    let value = object.get(scope, value_key).unwrap();
    <Option<T> as FromV8>::from_v8(scope, value)
}

#[macro_export]
macro_rules! js_impl_scoped_iterator {
    ($kind:ident $return_type:ty) => {
        pub fn $kind(
            &self,
            scope: &mut v8::HandleScope<'s>,
        ) -> $crate::sys::js::iterator::ScopedIterator<'s, $return_type> {
            let method_name = v8::String::new(scope, stringify!($kind)).unwrap();
            let js_iterator = $crate::sys::js::class::instance_call_method(scope, &self.0, method_name, &[])
                .try_cast()
                .unwrap();
            $crate::sys::js::iterator::ScopedIterator {
                js_iterator,
                marker: std::marker::PhantomData,
            }
        }
    };
}

#[macro_export]
macro_rules! js_impl_iterator {
    (entries $return_type:ty) => {
        $crate::js_impl_scoped_iterator!(entries (deno_core::ByteString, $return_type));
    };
    (values $return_type:ty) => {
        $crate::js_impl_scoped_iterator!(values $return_type);
    };
    (keys) => {
        $crate::js_impl_scoped_iterator!(keys deno_core::ByteString);
    }
}

#[macro_export]
macro_rules! js_entries {
    ($return_type:ty) => {
        $crate::js_impl_iterator!(entries $return_type);
    };
}

#[macro_export]
macro_rules! js_values {
    ($return_type:ty) => {
        $crate::js_impl_iterator!(values $return_type);
    };
}


#[macro_export]
macro_rules! js_keys {
    () => {
        $crate::js_impl_iterator!(keys);
    };
}

