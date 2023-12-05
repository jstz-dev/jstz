use std::{marker::PhantomData, ops::Deref};

use boa_engine::{
    object::builtins::JsFunction, value::TryFromJs, Context, JsResult, JsValue,
};
use boa_gc::{custom_trace, Finalize, Trace};

use crate::value::IntoJs;

pub trait IntoJsArgs {
    type Target: AsRef<[JsValue]>;
    fn into_js_args(self, context: &mut Context<'_>) -> Self::Target;
}

impl IntoJsArgs for () {
    type Target = [JsValue; 0];
    fn into_js_args(self, _context: &mut Context<'_>) -> Self::Target {
        []
    }
}

impl<T0: IntoJs> IntoJsArgs for (T0,) {
    type Target = [JsValue; 1];
    fn into_js_args(self, context: &mut Context<'_>) -> Self::Target {
        [self.0.into_js(context)]
    }
}

impl<T0: IntoJs, T1: IntoJs> IntoJsArgs for (T0, T1) {
    type Target = [JsValue; 2];
    fn into_js_args(self, context: &mut Context<'_>) -> Self::Target {
        [self.0.into_js(context), self.1.into_js(context)]
    }
}

impl<T0: IntoJs, T1: IntoJs, T2: IntoJs> IntoJsArgs for (T0, T1, T2) {
    type Target = [JsValue; 3];
    fn into_js_args(self, context: &mut Context<'_>) -> Self::Target {
        [
            self.0.into_js(context),
            self.1.into_js(context),
            self.2.into_js(context),
        ]
    }
}

/// A `JsFn<T, I, O>` is a `JsFunction` tagged with some Rust types used to handle the `TryFromJs` and `IntoJs` conversions automatically:
/// - `T` is the type of the `this` parameter;
/// - `N` is the arity;
/// - `I` is a tuple `(I1, ..., IN)` that contains the types of the parameters;
/// - `O` is the type of the output.
#[derive(Debug)]
pub struct JsFn<T: IntoJs, I: IntoJsArgs, O: TryFromJs> {
    function: JsFunction,
    _this_type: PhantomData<T>,
    _inputs_type: PhantomData<I>,
    _output_type: PhantomData<O>,
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> Finalize for JsFn<T, I, O> {}

unsafe impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> Trace for JsFn<T, I, O> {
    custom_trace!(this, {
        mark(&this.function);
    });
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> Deref for JsFn<T, I, O> {
    type Target = JsFunction;

    fn deref(&self) -> &Self::Target {
        &self.function
    }
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> From<JsFn<T, I, O>> for JsFunction {
    fn from(value: JsFn<T, I, O>) -> Self {
        value.function
    }
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> From<JsFunction> for JsFn<T, I, O> {
    fn from(value: JsFunction) -> Self {
        JsFn {
            function: value,
            _this_type: PhantomData,
            _inputs_type: PhantomData,
            _output_type: PhantomData,
        }
    }
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> From<JsFn<T, I, O>> for JsValue {
    fn from(value: JsFn<T, I, O>) -> Self {
        value.function.into()
    }
}

// impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> TryFrom<JsValue> for JsFn<T, I, O>
// This is implementable, but the right way to implement it would be to lift the implementation of `TryFromJs` for `JsFunction` (that does not use the context) to an implementation of `TryFrom<JsFunction>` in boa
// (If it is eventually implemented, then the implementation of TryFromJs below should use it)

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> IntoJs for JsFn<T, I, O> {
    fn into_js(self, _context: &mut Context<'_>) -> JsValue {
        self.function.into()
    }
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> TryFromJs for JsFn<T, I, O> {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        JsFunction::try_from_js(value, context).map(JsFn::from)
    }
}

impl<T: IntoJs, I: IntoJsArgs, O: TryFromJs> JsFn<T, I, O> {
    pub fn call(&self, this: T, inputs: I, context: &mut Context<'_>) -> JsResult<O> {
        let js_this = this.into_js(context);
        let js_args = inputs.into_js_args(context);
        self.deref()
            .call(&js_this, js_args.as_ref(), context)
            .and_then(|output| O::try_from_js(&output, context))
    }
}
