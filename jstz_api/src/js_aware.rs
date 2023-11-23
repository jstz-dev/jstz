use std::{convert, ops};

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::NativeObject;
use boa_engine::{object::builtins::JsArray, *};
use boa_gc::Trace;
use derive_more::*;
use enum_as_inner::EnumAsInner;
use jstz_core::native::JsNativeObject;
use jstz_core::value::*;

// Types

#[derive(EnumAsInner, PartialEq)]
pub enum JsOptional<T> {
    Defined(T),
    Undefined,
}

#[derive(EnumAsInner, PartialEq)]
pub enum JsNullable<T> {
    NonNull(T),
    Null,
}

// Traits

pub trait JsUndefinedAware {
    fn is_undefined(&self) -> bool;
}

pub trait JsNullAware {
    fn is_null(&self) -> bool;
}

// From impls

impl<T> From<T> for JsOptional<T>
where
    T: JsUndefinedAware,
{
    fn from(value: T) -> Self {
        if value.is_undefined() {
            JsOptional::Undefined
        } else {
            JsOptional::Defined(value)
        }
    }
}

impl<T> From<T> for JsNullable<T>
where
    T: JsNullAware,
{
    fn from(value: T) -> Self {
        if value.is_null() {
            JsNullable::Null
        } else {
            JsNullable::NonNull(value)
        }
    }
}

// JsTypeAware

pub trait JsTypeAware {
    fn get_type(&self) -> Type;
}

impl<T> JsUndefinedAware for T
where
    T: JsTypeAware,
{
    fn is_undefined(&self) -> bool {
        self.get_type() == Type::Undefined
    }
}

impl<T> JsNullAware for T
where
    T: JsTypeAware,
{
    fn is_null(&self) -> bool {
        self.get_type() == Type::Null
    }
}

impl<T> JsTypeAware for JsOptional<T>
where
    T: JsTypeAware,
{
    fn get_type(&self) -> Type {
        match self {
            JsOptional::Defined(value) => value.get_type(),
            JsOptional::Undefined => Type::Undefined,
        }
    }
}

impl<T> JsTypeAware for JsNullable<T>
where
    T: JsTypeAware,
{
    fn get_type(&self) -> Type {
        match self {
            JsNullable::NonNull(value) => value.get_type(),
            JsNullable::Null => Type::Null,
        }
    }
}

#[macro_export]
macro_rules! impl_JsTypeAware {
    ($typ:ty, $expr:expr) => {
        impl JsTypeAware for $typ {
            fn get_type(&self) -> Type {
                $expr
            }
        }
    };
}

impl JsTypeAware for JsValue {
    fn get_type(&self) -> Type {
        JsValue::get_type(self)
    }
}

impl_JsTypeAware!(u64, Type::Number);
impl_JsTypeAware!(f64, Type::Number);
impl_JsTypeAware!(JsObject, Type::Object);
impl_JsTypeAware!(JsFunction, Type::Object);

impl<T> JsTypeAware for JsNativeObject<T>
where
    T: NativeObject,
{
    fn get_type(&self) -> Type {
        Type::Object
    }
}

impl_JsTypeAware!(
    crate::stream::readable::internals::types_for_readable_stream::ReadableStreamReader,
    Type::Object
); // TODO move

impl_JsTypeAware!(
    crate::stream::readable::internals::underlying_source::ReadableStreamType,
    Type::String
); // TODO move

// TryFromJs impls

impl<T> TryFromJs for JsOptional<T>
where
    T: TryFromJs + JsTypeAware,
{
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if value.is_undefined() {
            Ok(JsOptional::Undefined)
        } else {
            T::try_from_js(value, context).map(Into::into)
        }
    }
}

impl<T> TryFromJs for JsNullable<T>
where
    T: TryFromJs + JsTypeAware,
{
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        if value.is_null() {
            Ok(JsNullable::Null)
        } else {
            T::try_from_js(value, context).map(Into::into)
        }
    }
}

// Into<JsValue> impls

impl<T> Into<JsValue> for JsOptional<T>
where
    T: Into<JsValue> + JsTypeAware,
{
    fn into(self) -> JsValue {
        match self {
            JsOptional::Defined(value) => value.into(),
            JsOptional::Undefined => JsValue::Undefined,
        }
    }
}

impl<T> Into<JsValue> for JsNullable<T>
where
    T: Into<JsValue> + JsTypeAware,
{
    fn into(self) -> JsValue {
        match self {
            JsNullable::NonNull(value) => value.into(),
            JsNullable::Null => JsValue::Undefined,
        }
    }
}

// IntoJs impls

impl<T> IntoJs for JsOptional<T>
where
    T: IntoJs + JsTypeAware,
{
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        match self {
            JsOptional::Defined(value) => value.into_js(context),
            JsOptional::Undefined => JsValue::Undefined,
        }
    }
}

impl<T> IntoJs for JsNullable<T>
where
    T: IntoJs + JsTypeAware,
{
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        match self {
            JsNullable::NonNull(value) => value.into_js(context),
            JsNullable::Null => JsValue::Undefined,
        }
    }
}

// Default impls

impl<T> Default for JsOptional<T> {
    fn default() -> Self {
        JsOptional::Undefined
    }
}

impl<T> Default for JsNullable<T> {
    fn default() -> Self {
        JsNullable::Null
    }
}
