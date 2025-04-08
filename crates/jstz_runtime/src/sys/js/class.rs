//! This module provides bindings for calling JS from Rust. It provides an
//! ergonomic way to call JS functions and methods exposed on the global object
//! on an "as-need" basis. To provide type safety, arguments and return types must
//! implement [`ToV8`] and [`FromV8`] respectively.
//!
//! Note that it is the responsibility of the developer to ensure that the defined
//! Rust structure is coherent with the JS structure. For example, the following code
//! defines a binding on the class `Foo`.
//!
//! ```
//! js_class!(Foo)
//!
//! impl <'s> Foo<'s> {
//!     js_constructor! { fn new(init: FooInit<'s>, data: FooData) }
//!
//!     js_method! { fn bar() -> u64 }
//! }
//! ```
//! The developer has to ensure that `Foo` has a constructor that accepts the positional
//! arguments `FooInit` and `FooData`. Additionally, the developer must ensure the method
//! `bar` exists and that it takes no arguments and returns a u64 number.
//!
//! TODO: Handle runtime type errors
//! https://linear.app/tezos/issue/JSTZ-454/handle-runtime-type-errors-in-the-web-sys-macro
//!

use deno_core::v8;

use crate::sys::worker_global_scope;

pub(crate) fn class_object<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
) -> v8::Local<'s, v8::Object> {
    let worker_global_scope = worker_global_scope(scope);

    worker_global_scope
        .0
        .get(scope, class_name.into())
        .unwrap()
        .try_cast()
        .unwrap()
}

pub(crate) fn constructor<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
) -> v8::Local<'s, v8::Function> {
    class_object(scope, class_name).try_cast().unwrap()
}

pub(crate) fn new_instance<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> v8::Local<'s, v8::Object> {
    let constructor = constructor(scope, class_name);

    constructor.new_instance(scope, args).unwrap()
}

pub(crate) fn instance_call_method<'s>(
    scope: &mut v8::HandleScope<'s>,
    this: &v8::Local<v8::Object>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> v8::Local<'s, v8::Value> {
    instance_call_method_with_recv(scope, this, (*this).into(), method_name, args)
}

pub(crate) fn instance_call_method_with_recv<'s>(
    scope: &mut v8::HandleScope<'s>,
    this: &v8::Local<v8::Object>,
    recv: v8::Local<v8::Value>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> v8::Local<'s, v8::Value> {
    let method_fn = this.get(scope, method_name.into()).unwrap();
    let method_fn = v8::Local::<v8::Function>::try_from(method_fn).unwrap();

    method_fn.call(scope, recv, args).unwrap()
}

pub(crate) fn static_call_method<'s>(
    scope: &mut v8::HandleScope<'s>,
    class_name: v8::Local<v8::String>,
    method_name: v8::Local<v8::String>,
    args: &[v8::Local<v8::Value>],
) -> v8::Local<'s, v8::Value> {
    let this = class_object(scope, class_name);
    let recv = worker_global_scope(scope).0;

    instance_call_method_with_recv(scope, &this, recv.into(), method_name, args)
}

pub(crate) trait JsClass {
    const JS_CLASS_NAME: deno_core::FastStaticString;
}

#[macro_export]
macro_rules! js_method {
    (
        #[js_name($js_name:ident)]
        fn $method_name:ident($($method_arg_name:ident: $method_arg_type:ty),*) $(-> $method_return:ty)?
    ) => {
        pub fn $method_name(
            &self,
            scope: &mut v8::HandleScope<'s>,
            $($method_arg_name: $method_arg_type),*
        ) $(-> $method_return)? {
            let method_name = v8::String::new(scope, stringify!($js_name)).unwrap();
            let args =
                [
                $(<$method_arg_type as $crate::sys::js::convert::ToV8>::to_v8($method_arg_name, scope)),*
                ];

            #[allow(unused)]
            let result = $crate::sys::js::class::instance_call_method(scope, &self.0, method_name, &args);

            $(
                <$method_return as $crate::sys::js::convert::FromV8>::from_v8(scope, result)
            )?
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
            #[js_name($method_name)]
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
        pub fn $method_name(
            scope: &mut v8::HandleScope<'s>,
            $($method_arg_name: $method_arg_type),*
        ) $(-> $method_return)?  {
            let class_name = <Self as $crate::sys::js::class::JsClass>::JS_CLASS_NAME.v8_string(scope).unwrap();
            let method_name = v8::String::new(scope, stringify!($js_name)).unwrap();
            let args =
                [
                $(<$method_arg_type as $crate::sys::js::convert::ToV8>::to_v8($method_arg_name, scope)),*
                ];

            #[allow(unused)]
            let result = $crate::sys::js::class::static_call_method(scope, class_name, method_name, &args);

            $(
                <$method_return as $crate::sys::js::convert::FromV8>::from_v8(scope, result)
            )?
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
        ) -> Self {
            let class_name = <Self as $crate::sys::js::class::JsClass>::JS_CLASS_NAME.v8_string(scope).unwrap();
            let args = [
                $(<$constructor_arg_type as $crate::sys::js::convert::ToV8>::to_v8($constructor_arg_name, scope)),*
            ];
            Self($crate::sys::js::class::new_instance(scope, class_name, &args))
        }
    };
}

#[macro_export]
macro_rules! js_getter {
    (
        #[js_name($js_name:ident)]
        fn $getter_name:ident() -> $getter_return:ty
    ) => {
        pub fn $getter_name(&self, scope: &mut v8::HandleScope<'s>) -> $getter_return {
            let getter_name = v8::String::new(scope, stringify!($js_name)).unwrap();
            let result = self.0.get(scope, getter_name.into()).unwrap();
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
    (
        async fn $getter_name:ident() -> $getter_return:ty
    ) => {
        $crate::js_getter! {
            #[js_name($getter_name)]
            fn $getter_name() -> $crate::sys::js::promise::Promise<$getter_return>
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
        ) {
            let setter_name = v8::String::new(scope, stringify!($js_name)).unwrap();
            let value =
                <$arg_type as $crate::sys::js::convert::ToV8>::to_v8($arg_name, scope);
            self.0.set(scope, setter_name.into(), value).unwrap();
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
        // Define the struct for instances
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name<'a>(v8::Local<'a, v8::Object>);

        impl<'a> $crate::sys::js::convert::FromV8<'a> for $name<'a> {
            fn from_v8(
                scope: &mut v8::HandleScope<'a>,
                value: v8::Local<'a, v8::Value>,
            ) -> Self {
                Self(value.to_object(scope).unwrap())
            }
        }

        impl<'a> $crate::sys::js::convert::ToV8<'a> for $name<'a> {
            fn to_v8(self, _scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Value> {
                self.0.into()
            }
        }

        impl<'s> $crate::sys::js::class::JsClass for $name<'s> {
            const JS_CLASS_NAME: deno_core::FastStaticString =
                deno_core::ascii_str!(stringify!($name));
        }
    };
}
