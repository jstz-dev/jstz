#![allow(unused)]
use deno_core::v8;
use std::marker::PhantomData;

use crate::JstzRuntime;

use super::convert::FromV8;

/// Bindings for JS Promise that resolves to T
pub struct Promise<T>(v8::Global<v8::Value>, PhantomData<T>);

impl<'s, T: FromV8<'s>> Promise<T> {
    pub fn new(value: v8::Global<v8::Value>) -> Self {
        Self(value, PhantomData)
    }

    /// Drives the promise with `runtime`
    pub async fn with_runtime(self, runtime: &'s mut JstzRuntime) -> T {
        let promise = runtime.resolve(self.0);
        let result = runtime
            .with_event_loop_future(promise, Default::default())
            .await
            .unwrap();
        let scope = &mut runtime.handle_scope();
        let result = v8::Local::new(scope, result);
        T::from_v8(scope, result)
    }
}

impl<'s, T: FromV8<'s>> FromV8<'s> for Promise<T> {
    fn from_v8(scope: &mut v8::HandleScope<'s>, value: v8::Local<'s, v8::Value>) -> Self {
        let promise = v8::Global::new(scope, value);
        Self::new(promise)
    }
}
