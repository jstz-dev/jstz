//! This module provides traits and macros that facilitate the conversion of Rust objects into v8 objects
//! (and vice versa)
//!
//! This set of conversion traits combines code from `deno_core::convert` and `deno_core::ops`'s conversion
//! functions. We have to use our own traits due to Rust's orphan rule. Additional `deno_core::ops`'s conversions
//! are generated using a procedural macro (not exposed by the crate).

use std::borrow::Cow;

use deno_core::{
    serde_v8::{self, V8Sliceable},
    v8,
};

pub trait FromV8<'a> {
    fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Self;
}

pub trait ToV8<'a> {
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value>;
}

impl<'a, T> ToV8<'a> for Option<T>
where
    T: ToV8<'a>,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        match self {
            Some(value) => value.to_v8(scope),
            None => v8::null(scope).into(),
        }
    }
}

impl<'a, T> FromV8<'a> for Option<T>
where
    T: FromV8<'a>,
{
    fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Self {
        if value.is_null_or_undefined() {
            None
        } else {
            Some(T::from_v8(scope, value))
        }
    }
}

/// Helper macro for [`ToV8`] to reduce some boilerplate.
macro_rules! impl_to_v8 {
  (for $( $t: ty ),+ where |$value:ident, $scope:ident| $block:expr) => {
    $(
      impl<'a> ToV8<'a> for $t {
        fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
          let $value = self;
          let $scope = scope;
          v8::Local::<v8::Value>::from($block)
        }
      }
    )+
  };
  ($t: ty where |$value: ident, $scope:ident| $block:expr) => {
    impl_to_v8!(for $t where |$value, $scope| $block);
  };
}

macro_rules! impl_from_v8 {
    (for $( $t: ty ),+ where |$value:ident, $scope:ident| $block:expr) => {
        $(
            impl<'a> FromV8<'a> for $t {
                fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Self {
                    let $value = value;
                    let $scope = scope;
                    $block
                }
            }
        )+
    };
    ($t: ty where |$value: ident, $scope:ident| $block:expr) => {
        impl_from_v8!(for $t where |$value, $scope| $block);
    };
}

// The Rust unit type `()` maps to v8's `null` primitive
impl_to_v8!(() where |_value, scope| v8::null(scope));

// The Rust `bool` type maps to `v8::Boolean`
impl_to_v8!(bool where |value, scope| v8::Boolean::new(scope, value));
impl_from_v8!(bool where |value, _scope| value.is_true());

// Integers

impl_to_v8!(for u8, u16, u32 where |value, scope| v8::Integer::new_from_unsigned(scope, value as _));
impl_from_v8!(for u8, u16, u32 where |value, _scope| deno_core::_ops::to_i32_option(&value).unwrap() as _);

impl_to_v8!(for i8, i16, i32 where |value, scope| v8::Integer::new(scope, value as _));
impl_from_v8!(for i8, i16, i32 where |value, _scope| deno_core::_ops::to_i32_option(&value).unwrap() as _);

impl_to_v8!(for u64, usize where |value, scope| v8::BigInt::new_from_u64(scope, value as _));
impl_from_v8!(for u64, usize where |value, _scope| deno_core::_ops::to_u64_option(&value).unwrap() as _);

impl_to_v8!(for i64, isize where |value, scope| v8::BigInt::new_from_i64(scope, value as _));
impl_from_v8!(for i64, isize where |value, _scope| deno_core::_ops::to_i64_option(&value).unwrap() as _);

// Floats
impl_to_v8!(for f32, f64 where |value, scope| v8::Number::new(scope, value as _));
impl_from_v8!(for f32, f64 where |value, _scope| deno_core::_ops::to_f64_option(&value).unwrap() as _);

// Strings
impl_to_v8!(for String, Cow<'a, str>, &'a str where |value, scope| v8::String::new(scope, &value).unwrap());
impl_from_v8!(for String where |value, scope| value.to_rust_string_lossy(scope));
impl_to_v8!(for deno_core::ByteString where |value, scope| serde_v8::to_v8(scope, value).unwrap());
impl_from_v8!(for deno_core::ByteString where |value, scope| serde_v8::from_v8(scope, value).unwrap());

// Buffers

/// A wrapper type for `V8Slice<T>` that ignores the underlying range, (de)serialising as
/// an opaque `ArrayBuffer`
#[derive(Debug, Clone)]
pub struct ArrayBuffer<T: V8Sliceable>(pub serde_v8::V8Slice<T>);

impl<'a, T> ToV8<'a> for serde_v8::V8Slice<T>
where
    T: V8Sliceable,
    v8::Local<'a, v8::Value>: From<v8::Local<'a, T::V8>>,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        v8::Local::<v8::Value>::from(self.into_v8_local(scope).unwrap())
    }
}

impl<'a, T> ToV8<'a> for ArrayBuffer<T>
where
    T: V8Sliceable,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        v8::Local::<v8::Value>::from(self.0.into_v8_unsliced_arraybuffer_local(scope))
    }
}

impl<'a> ToV8<'a> for serde_v8::JsBuffer {
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        self.into_parts().to_v8(scope)
    }
}

impl<'a> FromV8<'a> for serde_v8::JsBuffer {
    fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Self {
        serde_v8::from_v8(scope, value).unwrap()
    }
}
// v8::Local types

impl<'a, T> FromV8<'a> for v8::Local<'a, T>
where
    v8::Local<'a, T>: TryFrom<v8::Local<'a, v8::Value>>,
{
    fn from_v8(
        _scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Self {
        v8::Local::<T>::try_from(value).map_err(drop).unwrap()
    }
}

impl<'a, T> ToV8<'a> for v8::Local<'a, T>
where
    v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
    fn to_v8(self, _scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        self.into()
    }
}

// Serde

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Serde<T>(pub T);

impl<T> From<T> for Serde<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<'a, T> ToV8<'a> for Serde<T>
where
    T: serde::Serialize,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
        serde_v8::to_v8(scope, self.0).unwrap()
    }
}

impl<'a, T> FromV8<'a> for Serde<T>
where
    T: serde::Deserialize<'a>,
{
    fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Self {
        // TODO: Handle errors
        Serde(serde_v8::from_v8(scope, value).unwrap())
    }
}

impl<'s, T, U> FromV8<'s> for (T, U)
where
    T: FromV8<'s>,
    U: FromV8<'s>,
{
    fn from_v8(scope: &mut v8::HandleScope<'s>, value: v8::Local<'s, v8::Value>) -> Self {
        let object = value.try_cast::<v8::Object>().unwrap();
        let fs_key = v8::Integer::new(scope, 0);
        let fs = object.get(scope, fs_key.into()).unwrap();
        let snd_key = v8::Integer::new(scope, 1);
        let snd = object.get(scope, snd_key.into()).unwrap();
        (T::from_v8(scope, fs), U::from_v8(scope, snd))
    }
}
