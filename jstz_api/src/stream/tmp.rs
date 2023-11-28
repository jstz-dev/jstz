use std::{marker::PhantomData, ops::Deref};

use boa_engine::{
    object::builtins::JsFunction, value::TryFromJs, Context, JsResult, JsValue,
};
use jstz_core::value::IntoJs;

use boa_gc::{custom_trace, Finalize, Trace};

pub enum Todo {} // TODO remove

#[macro_export]
macro_rules! todo_boa_type {
    ( $t:ident ) => {
        pub struct $t;

        impl Finalize for $t {
            fn finalize(&self) {
                todo!()
            }
        }

        unsafe impl Trace for $t {
            custom_trace!(this, { todo!() });
        }
    };
}

todo_boa_type!(ReadableStreamDefaultController);

todo_boa_type!(ReadableByteStreamController);

pub struct JsUnaryFunction<T: IntoJs, I: IntoJs, O: TryFromJs> {
    function: JsFunction,
    _this_type: PhantomData<T>,
    _input_type: PhantomData<I>,
    _output_type: PhantomData<O>,
}

impl<T: IntoJs, I: IntoJs, O: TryFromJs> Deref for JsUnaryFunction<T, I, O> {
    type Target = JsFunction;

    fn deref(&self) -> &Self::Target {
        &self.function
    }
}

impl<T: IntoJs, I: IntoJs, O: TryFromJs> IntoJs for JsUnaryFunction<T, I, O> {
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        self.function.into_js(context)
    }
}

impl<T: IntoJs, I: IntoJs, O: TryFromJs> From<JsFunction> for JsUnaryFunction<T, I, O> {
    fn from(value: JsFunction) -> Self {
        JsUnaryFunction {
            function: value,
            _this_type: PhantomData,
            _input_type: PhantomData,
            _output_type: PhantomData,
        }
    }
}

impl<T: IntoJs, I: IntoJs, O: TryFromJs> TryFromJs for JsUnaryFunction<T, I, O> {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        JsFunction::try_from_js(value, context).map(JsUnaryFunction::from)
    }
}

impl<T: IntoJs, I: IntoJs, O: TryFromJs> JsUnaryFunction<T, I, O> {
    pub fn call(&self, this: T, input: I, context: &mut Context<'_>) -> JsResult<O> {
        let js_this = this.into_js(context);
        let js_arg = input.into_js(context);
        let js_args = [js_arg];
        self.deref()
            .call(&js_this, &js_args, context)
            .and_then(|output| O::try_from_js(&output, context))
    }
}
