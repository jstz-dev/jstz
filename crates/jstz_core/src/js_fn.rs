use std::{marker::PhantomData, ops::Deref};

use boa_engine::{
    object::builtins::JsFunction, value::TryFromJs, Context, JsResult, JsValue,
};
use boa_gc::{custom_trace, Finalize, Trace};

use crate::value::IntoJs;

pub trait IntoJsArgs<const N: usize> {
    fn into_js_args(self, context: &mut Context<'_>) -> [JsValue; N];
}

impl IntoJsArgs<0> for () {
    fn into_js_args(self, _context: &mut Context<'_>) -> [JsValue; 0] {
        []
    }
}

impl<T0: IntoJs> IntoJsArgs<1> for (T0,) {
    fn into_js_args(self, context: &mut Context<'_>) -> [JsValue; 1] {
        [self.0.into_js(context)]
    }
}

impl<T0: IntoJs, T1: IntoJs> IntoJsArgs<2> for (T0, T1) {
    fn into_js_args(self, context: &mut Context<'_>) -> [JsValue; 2] {
        [self.0.into_js(context), self.1.into_js(context)]
    }
}

impl<T0: IntoJs, T1: IntoJs, T2: IntoJs> IntoJsArgs<3> for (T0, T1, T2) {
    fn into_js_args(self, context: &mut Context<'_>) -> [JsValue; 3] {
        [
            self.0.into_js(context),
            self.1.into_js(context),
            self.2.into_js(context),
        ]
    }
}

/// A `JsFn<T, N, I, O>` is a `JsFunction` tagged with some Rust types used to handle the `TryFromJs` and `IntoJs` conversions automatically:
/// - `T` is the type of the `this` parameter;
/// - `N` is the arity;
/// - `I` is a tuple `(I1, ..., IN)` that contains the types of the parameters;
/// - `O` is the type of the output.
#[derive(Debug)]
pub struct JsFn<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> {
    function: JsFunction,
    _this_type: PhantomData<T>,
    _inputs_type: PhantomData<I>,
    _output_type: PhantomData<O>,
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> Finalize
    for JsFn<T, N, I, O>
{
    fn finalize(&self) {}
}

unsafe impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> Trace
    for JsFn<T, N, I, O>
{
    custom_trace!(this, {
        mark(&this.function);
    });
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> Deref
    for JsFn<T, N, I, O>
{
    type Target = JsFunction;

    fn deref(&self) -> &Self::Target {
        &self.function
    }
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> Into<JsFunction>
    for JsFn<T, N, I, O>
{
    fn into(self) -> JsFunction {
        self.function
    }
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> From<JsFunction>
    for JsFn<T, N, I, O>
{
    fn from(value: JsFunction) -> Self {
        JsFn {
            function: value,
            _this_type: PhantomData,
            _inputs_type: PhantomData,
            _output_type: PhantomData,
        }
    }
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> Into<JsValue>
    for JsFn<T, N, I, O>
{
    fn into(self) -> JsValue {
        self.function.into()
    }
}

// impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> TryFrom<JsValue> for JsFn<T, N, I, O>
// This is implementable, but the right way to implement it would be to lift the implementation of `TryFromJs` for `JsFunction` (that does not use the context) to an implementation of `TryFrom<JsFunction>` in boa
// (If it is eventually implemented, then the implementation of TryFromJs below should use it)

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> IntoJs
    for JsFn<T, N, I, O>
{
    fn into_js(self, _context: &mut Context<'_>) -> JsValue {
        self.function.into()
    }
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> TryFromJs
    for JsFn<T, N, I, O>
{
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        JsFunction::try_from_js(value, context).map(JsFn::from)
    }
}

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> JsFn<T, N, I, O> {
    pub fn call(&self, this: T, inputs: I, context: &mut Context<'_>) -> JsResult<O> {
        let js_this = this.into_js(context);
        let js_args = inputs.into_js_args(context);
        self.deref()
            .call(&js_this, &js_args, context)
            .and_then(|output| O::try_from_js(&output, context))
    }
}
