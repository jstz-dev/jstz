//! This module provides traits and macros that facilitate the conversion of
//! Rust objects into v8 objects (and vice versa)
//!
//! When passing data from Rust into JS using the web-sys classes,
//! we need to (de)serialize the data into a native V8 value. The auto-generated
//! bindings will implicitly use the `FromV8` and `ToV8` traits to do so.
//!
//! # Example
//! ```notrust
//! use deno_core::v8;
//! use jstz_runtime::sys::js::convert::ToV8;
//!
//! struct Response<'s>(v8::Local<'s, v8::Object>);
//!
//! impl<'s> ToV8<'s> for Response<'s> {
//!    fn to_v8(self, scope: &mut v8::HandleScope<'s>) -> Result<v8::Local<'s, v8::Value>> {
//!        Ok(self.0.into())
//!    }
//! }
//! ```
//!
//! # Why not use `deno_core::convert`?
//!
//! `deno_core::convert` provides `ToV8` and `FromV8` traits. However, only a few
//! types implement these traits. Due to Rust's orphan rule, we cannot provide our
//! own implementations to these trait (without wrapping the types).

use deno_core::{
    serde_v8::{self, V8Sliceable},
    v8, ByteString,
};
use derive_more::{Deref, DerefMut, From};
use std::borrow::Cow;

use crate::error::{Result, RuntimeError};

use super::class::instance_get;

/// A conversion from a v8 value to a rust value.
pub trait FromV8<'a>: Sized {
    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self>;
}

/// A conversion from a rust value to a v8 value.
pub trait ToV8<'a> {
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>>;
}

impl<'a, T> ToV8<'a> for Option<T>
where
    T: ToV8<'a>,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        match self {
            Some(value) => value.to_v8(scope),
            None => Ok(v8::null(scope).into()),
        }
    }
}

impl<'a, T> FromV8<'a> for Option<T>
where
    T: FromV8<'a>,
{
    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self> {
        if value.is_null_or_undefined() {
            Ok(None)
        } else {
            Ok(Some(T::from_v8(scope, value)?))
        }
    }
}

/// Helper macro for [`ToV8`] to reduce some boilerplate.
macro_rules! impl_to_v8 {
  (for $( $t: ty ),+ where |$value:ident, $scope:ident| $block:expr) => {
    $(
      impl<'a> ToV8<'a> for $t {
        fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
          let $value = self;
          let $scope = scope;
          Ok(v8::Local::<v8::Value>::from($block))
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
                fn from_v8(scope: &mut v8::HandleScope<'a>, value: v8::Local<'a, v8::Value>) -> Result<Self> {
                    let $value = value;
                    let $scope = scope;
                    Ok($block)
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
impl_from_v8!(for u8, u16, u32 where |value, _scope| 
    deno_core::_ops::to_u32_option(&value)
        .ok_or_else(|| RuntimeError::type_error("Expected u32"))? as _);

impl_to_v8!(for i8, i16, i32 where |value, scope| v8::Integer::new(scope, value as _));
impl_from_v8!(for i8, i16, i32 where |value, _scope| 
    deno_core::_ops::to_i32_option(&value)
        .ok_or_else(|| RuntimeError::type_error("Expected i32"))? as _);

impl_to_v8!(for u64, usize where |value, scope| v8::BigInt::new_from_u64(scope, value as _));
impl_from_v8!(for u64, usize where |value, _scope| 
    deno_core::_ops::to_u64_option(&value)
        .ok_or_else(|| RuntimeError::type_error("Expected u64"))? as _);

impl_to_v8!(for i64, isize where |value, scope| v8::BigInt::new_from_i64(scope, value as _));
impl_from_v8!(for i64, isize where |value, _scope| 
    deno_core::_ops::to_i64_option(&value)
        .ok_or_else(|| RuntimeError::type_error("Expected i64"))? as _);

// Floats
impl_to_v8!(for f32, f64 where |value, scope| v8::Number::new(scope, value as _));
impl_from_v8!(for f32, f64 where |value, _scope| 
    deno_core::_ops::to_f64_option(&value)
        .ok_or_else(|| RuntimeError::type_error("Expected f64"))? as _);

// Strings

impl_to_v8!(for String, Cow<'a, str> where |value, scope| 
    v8::String::new(scope, &value)
        .ok_or_else(|| RuntimeError::cannot_alloc("String"))?);

impl_to_v8!(for &'a str where |value, scope| 
    v8::String::new(scope, value)
        .ok_or_else(|| RuntimeError::cannot_alloc("String"))?);

impl_to_v8!(for ByteString where |value, scope| Serde(value).to_v8(scope)?);
impl_from_v8!(for ByteString where |value, scope| Serde::<ByteString>::from_v8(scope, value)?.0);
impl_from_v8!(for String where |value, scope| value.to_rust_string_lossy(scope));

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
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        Ok(v8::Local::<v8::Value>::from(
            self.into_v8_local(scope)
                .ok_or_else(|| RuntimeError::cannot_alloc("Buffer"))?,
        ))
    }
}

impl<'a, T> ToV8<'a> for ArrayBuffer<T>
where
    T: V8Sliceable,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        Ok(v8::Local::<v8::Value>::from(
            self.0.into_v8_unsliced_arraybuffer_local(scope),
        ))
    }
}

impl<'a> ToV8<'a> for serde_v8::JsBuffer {
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        self.into_parts().to_v8(scope)
    }
}

impl<'a> FromV8<'a> for serde_v8::JsBuffer {
    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self> {
        Ok(serde_v8::from_v8(scope, value)?)
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
    ) -> Result<Self> {
        v8::Local::<T>::try_from(value)
            .map_err(|_| RuntimeError::type_error("Expected compatible `v8::Local` type"))
    }
}

impl<'a, T> ToV8<'a> for v8::Local<'a, T>
where
    v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
    fn to_v8(self, _scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        Ok(self.into())
    }
}

/// A wrapper type for `T` that (de)serializes using `serde_v8`.
#[derive(Deref, DerefMut, From, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Serde<T>(pub T);

impl<'a, T> ToV8<'a> for Serde<T>
where
    T: serde::Serialize,
{
    fn to_v8(self, scope: &mut v8::HandleScope<'a>) -> Result<v8::Local<'a, v8::Value>> {
        Ok(serde_v8::to_v8(scope, self.0)?)
    }
}

impl<'a, 'de, T> FromV8<'a> for Serde<T>
where
    T: serde::Deserialize<'de>,
{
    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self> {
        Ok(Serde(serde_v8::from_v8(scope, value)?))
    }
}

impl<'s, T, U> FromV8<'s> for (T, U)
where
    T: FromV8<'s>,
    U: FromV8<'s>,
{
    fn from_v8(
        scope: &mut v8::HandleScope<'s>,
        value: v8::Local<'s, v8::Value>,
    ) -> Result<Self> {
        let object = value.try_cast::<v8::Object>()?;
        let fs_key = v8::String::new(scope, "0").unwrap();
        let fs = instance_get(scope, &object, fs_key)?;
        let snd_key = v8::String::new(scope, "1").unwrap();
        let snd = instance_get(scope, &object, snd_key)?;
        Ok((T::from_v8(scope, fs)?, U::from_v8(scope, snd)?))
    }
}
