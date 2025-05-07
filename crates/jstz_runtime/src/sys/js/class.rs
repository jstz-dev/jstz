//! This module provides bindings for calling JS from Rust. It provides an
//! ergonomic way to call JS functions and methods exposed on the global object
//! on an "as-need" basis. To provide type safety, arguments and return types must
//! implement [`ToV8`] and [`FromV8`] respectively.
//!
//! Note that it is the responsibility of the developer to ensure that the defined
//! Rust structure is coherent with the JS structure. For example, the following code
//! defines a binding on the class `Foo`.
//!
//! # Example
//!
//! ```notrust
//! use jstz_runtime::{js_class, js_constructor, js_method};
//!
//! js_class!(Foo)
//!
//! impl <'s> Foo<'s> {
//!     js_constructor! { fn new(init: FooInit<'s>, data: FooData) }
//!
//!     js_method! { fn bar() -> u64 }
//! }
//! ```
//!
//! The developer has to ensure that `Foo` has a constructor that accepts the positional
//! arguments `FooInit` and `FooData`. Additionally, the developer must ensure the method
//! `bar` exists and that it takes no arguments and returns a u64 number.

use deno_core::v8;

use crate::{
    error::{Result, RuntimeError, WebSysError},
    sys::worker_global_scope,
};

pub(crate) fn class_object<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
) -> Result<v8::Local<'s, v8::Object>> {
    let worker_global_scope = worker_global_scope(scope);

    Ok(worker_global_scope
        .0
        .get(scope, class_name.into())
        .ok_or_else(|| WebSysError::ClassMissing(class_name.to_rust_string_lossy(scope)))?
        .try_cast()?)
}

pub(crate) fn constructor<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
) -> Result<v8::Local<'s, v8::Function>> {
    Ok(class_object(scope, class_name)?.try_cast()?)
}

pub(crate) fn new_instance<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> Result<v8::Local<'s, v8::Object>> {
    let constructor = constructor(scope, class_name)?;

    Ok(constructor.new_instance(scope, args).ok_or_else(|| {
        WebSysError::ConstructorFailed(class_name.to_rust_string_lossy(scope))
    })?)
}

pub(crate) fn instance_call_method<'s, T: JsClass>(
    scope: &mut v8::HandleScope<'s>,
    this: &v8::Local<v8::Object>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> Result<v8::Local<'s, v8::Value>> {
    instance_call_method_with_recv::<T>(scope, this, (*this).into(), method_name, args)
}

pub(crate) fn instance_call_method_with_recv<'s, T: JsClass>(
    scope: &mut v8::HandleScope<'s>,
    this: &v8::Local<v8::Object>,
    recv: v8::Local<v8::Value>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> Result<v8::Local<'s, v8::Value>> {
    let method_fn = this.get(scope, method_name.into()).ok_or_else(|| {
        WebSysError::MethodMissing {
            class_name: T::JS_CLASS_NAME.to_string(),
            method_name: method_name.to_rust_string_lossy(scope),
        }
    })?;
    let method_fn = v8::Local::<v8::Function>::try_from(method_fn)?;

    Ok(method_fn.call(scope, recv, args).ok_or_else(|| {
        WebSysError::MethodCallFailed(method_name.to_rust_string_lossy(scope))
    })?)
}

pub(crate) fn static_call_method<'s, T: JsClass>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> Result<v8::Local<'s, v8::Value>> {
    let this = class_object(scope, class_name)?;
    let recv = worker_global_scope(scope).0;

    instance_call_method_with_recv::<T>(scope, &this, recv.into(), method_name, args)
}

pub(crate) fn property_name<'s>(
    scope: &mut v8::HandleScope<'s>,
    name: &'static str,
) -> Result<v8::Local<'s, v8::String>> {
    v8::String::new(scope, name).ok_or_else(|| RuntimeError::cannot_alloc("String"))
}

pub(crate) fn instance_get<'s>(
    scope: &mut v8::HandleScope<'s>,
    this: &v8::Local<v8::Object>,
    property_name: v8::Local<v8::String>,
) -> Result<v8::Local<'s, v8::Value>> {
    Ok(this.get(scope, property_name.into()).ok_or_else(|| {
        WebSysError::PropertyMissing(property_name.to_rust_string_lossy(scope))
    })?)
}
pub(crate) fn instance_set(
    scope: &mut v8::HandleScope,
    this: &v8::Local<v8::Object>,
    property_name: v8::Local<v8::String>,
    value: v8::Local<v8::Value>,
) -> Result<()> {
    if let Some(true) = this.set(scope, property_name.into(), value) {
        Ok(())
    } else {
        Err(
            WebSysError::PropertySetFailed(property_name.to_rust_string_lossy(scope))
                .into(),
        )
    }
}

pub(crate) trait JsClass {
    const JS_CLASS_NAME: deno_core::FastStaticString;

    fn class_name<'s>(
        scope: &mut v8::HandleScope<'s>,
    ) -> Result<v8::Local<'s, v8::String>> {
        Self::JS_CLASS_NAME
            .v8_string(scope)
            .map_err(|_| RuntimeError::cannot_alloc("FastString"))
    }
}

#[macro_export]
macro_rules! js_method {
    (
        #[js_name($js_name:ident)]
        fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) $(-> $method_return:ty)?
    ) => {
        #[allow(unused_parens, clippy::needless_question_mark)]
        pub fn $method_name(
            &self,
            scope: &mut v8::HandleScope<'s>,
            $($method_arg_name: $method_arg_type),*
        ) -> $crate::error::Result<($($method_return)?)>
            where Self: std::ops::Deref<Target = v8::Local<'s, v8::Object>> {
            let method_name = $crate::sys::js::class::property_name(scope, stringify!($js_name))?;
            let args =
                [
                    $(<$method_arg_type as $crate::sys::js::convert::ToV8>::to_v8($method_arg_name, scope)?),*
                ];

            #[allow(unused)]
            let result = $crate::sys::js::class::instance_call_method::<Self>(scope, &self, method_name, &args)?;

            Ok(
                ($(<$method_return as $crate::sys::js::convert::FromV8>::from_v8(scope, result)?)?)
            )
        }
    };
    (
        fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) $(-> $method_return:ty)?
    ) => {
        $crate::js_method! {
            #[js_name($method_name)]
            fn $method_name($($method_arg_name: $method_arg_type),*) $(-> $method_return)?
        }
    };
    (
        #[js_name($js_name:ident)]
        async fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) -> $method_return:ty
    ) => {
        $crate::js_method! {
            #[js_name($js_name)]
            fn $method_name($($method_arg_name: $method_arg_type),*) -> $crate::sys::js::promise::Promise<$method_return>
        }
    };
    (
        async fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) -> $method_return:ty
    ) => {
        $crate::js_method! {
            #[js_name($method_name)]
            fn $method_name($($method_arg_name: $method_arg_type),*) -> $crate::sys::js::promise::Promise<$method_return>
        }
    };
}

#[macro_export]
macro_rules! js_static_method {
    (
        #[js_name($js_name:ident)]
        fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) $(-> $method_return:ty)?
    ) => {
        #[allow(unused_parens, clippy::needless_question_mark)]
        pub fn $method_name(
            scope: &mut v8::HandleScope<'s>,
            $($method_arg_name: $method_arg_type),*
        ) -> $crate::error::Result<($($method_return)?)>  {
            let class_name = <Self as $crate::sys::js::class::JsClass>::class_name(scope)?;
            let method_name = $crate::sys::js::class::property_name(scope, stringify!($js_name))?;
            let args =
                [
                    $(<$method_arg_type as $crate::sys::js::convert::ToV8>::to_v8($method_arg_name, scope)?),*
                ];

            #[allow(unused)]
            let result = $crate::sys::js::class::static_call_method::<Self>(scope, class_name, method_name, &args)?;

            Ok(
                ($(<$method_return as $crate::sys::js::convert::FromV8>::from_v8(scope, result)?)?)
            )
        }
    };
    (
        fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) $(-> $method_return:ty)?
    ) => {
        $crate::js_static_method! {
            #[js_name($method_name)]
            fn $method_name($($method_arg_name: $method_arg_type),*) $(-> $method_return)?
        }
    };
}

#[macro_export]
macro_rules! js_constructor {
    (
        fn $constructor_name:ident($($constructor_arg_name:ident: $constructor_arg_type:ty),*)
    ) => {
        pub fn $constructor_name(
            scope: &mut v8::HandleScope<'s>,
            $($constructor_arg_name: $constructor_arg_type),*
        ) -> $crate::error::Result<Self> {
            let class_name = <Self as $crate::sys::js::class::JsClass>::class_name(scope)?;
            let args = [
                $(<$constructor_arg_type as $crate::sys::js::convert::ToV8>::to_v8($constructor_arg_name, scope)?),*
            ];
            Ok(Self($crate::sys::js::class::new_instance(scope, class_name, &args)?))
        }
    };
}

#[macro_export]
macro_rules! js_getter {
    (
        #[js_name($js_name:ident)]
        fn $getter_name:ident() -> $getter_return:ty
    ) => {
        pub fn $getter_name(
            &self,
            scope: &mut v8::HandleScope<'s>,
        ) -> $crate::error::Result<$getter_return>
        where
            Self: std::ops::Deref<Target = v8::Local<'s, v8::Object>>,
        {
            let getter_name =
                $crate::sys::js::class::property_name(scope, stringify!($js_name))?;
            let result = $crate::sys::js::class::instance_get(scope, &self, getter_name)?;
            <$getter_return as $crate::sys::js::convert::FromV8>::from_v8(scope, result)
        }
    };
    (
        fn $getter_name:ident() -> $getter_return:ty
    ) => {
        $crate::js_getter! {
            #[js_name($getter_name)]
            fn $getter_name() -> $getter_return
        }
    };
}

#[macro_export]
macro_rules! js_setter {
    (
        #[js_name($js_name:ident)]
        fn $setter_name:ident($arg_name:ident: $arg_type:ty)
    ) => {
        pub fn $setter_name(
            &self,
            scope: &mut v8::HandleScope<'s>,
            $arg_name: $arg_type,
        ) -> $crate::error::Result<()> {
            let setter_name =
                $crate::sys::js::class::property_name(scope, stringify!($js_name))?;
            let value =
                <$arg_type as $crate::sys::js::convert::ToV8>::to_v8($arg_name, scope)?;
            $crate::sys::js::class::instance_set(scope, &self.0, setter_name, value)
        }
    };
    (
        fn $setter_name:ident($arg_name:ident: $arg_type:ty)
    ) => {
        $crate::js_setter! {
            #[js_name($setter_name)]
            fn $setter_name($arg_name: $arg_type)
        }
    };
}

#[macro_export]
macro_rules! js_class {
    (
        $name:ident
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name<'a>(v8::Local<'a, v8::Object>);

        impl<'a> $crate::sys::js::convert::FromV8<'a> for $name<'a> {
            fn from_v8(
                scope: &mut v8::HandleScope<'a>,
                value: v8::Local<'a, v8::Value>,
            ) -> $crate::error::Result<Self> {
                Ok(Self(value.to_object(scope).ok_or_else(|| {
                    $crate::error::RuntimeError::type_error("Expected `Object`")
                })?))
            }
        }

        impl<'a> $crate::sys::js::convert::ToV8<'a> for $name<'a> {
            fn to_v8(
                self,
                _scope: &mut v8::HandleScope<'a>,
            ) -> $crate::error::Result<v8::Local<'a, v8::Value>> {
                Ok(self.0.into())
            }
        }

        impl<'a> $crate::sys::js::class::JsClass for $name<'a> {
            const JS_CLASS_NAME: deno_core::FastStaticString =
                deno_core::ascii_str!(stringify!($name));
        }

        impl<'a> std::ops::Deref for $name<'a> {
            type Target = v8::Local<'a, v8::Object>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
