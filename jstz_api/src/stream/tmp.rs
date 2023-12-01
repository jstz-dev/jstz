use std::{marker::PhantomData, ops::Deref};

use boa_engine::{
    js_string, object::builtins::JsFunction, value::TryFromJs, Context, JsArgs, JsResult,
    JsValue, NativeFunction,
};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};

use boa_gc::{custom_trace, Finalize, Trace};

use super::abstractions::underlying_source::UnderlyingSource;

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

        impl IntoJs for $t {
            fn into_js(self, _context: &mut Context<'_>) -> JsValue {
                todo!()
            }
        }

        impl TryFromJs for $t {
            fn try_from_js(
                _value: &JsValue,
                _context: &mut Context<'_>,
            ) -> JsResult<Self> {
                todo!()
            }
        }
    };
}

todo_boa_type!(Todo);

todo_boa_type!(ReadableStreamDefaultController);

todo_boa_type!(ReadableByteStreamController);

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

impl<T: IntoJs, const N: usize, I: IntoJsArgs<N>, O: TryFromJs> IntoJs
    for JsFn<T, N, I, O>
{
    fn into_js(self, context: &mut Context<'_>) -> JsValue {
        self.function.into_js(context)
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

todo_boa_type!(TmpTest);
pub struct TmpTestClass {}
impl TmpTestClass {
    fn static_test(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        let arg_0 = args.get_or_undefined(0);
        let x = UnderlyingSource::try_from_js(arg_0, context)?;
        x.cancel(None, context);
        Ok(JsValue::Null)
    }
}

impl NativeClass for TmpTestClass {
    type Instance = TmpTest;

    const NAME: &'static str = "TmpTest";

    fn constructor(
        _this: &JsNativeObject<Self::Instance>,
        args: &[boa_engine::JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<Self::Instance> {
        todo!()
    }

    fn init(class: &mut ClassBuilder<'_, '_>) -> JsResult<()> {
        class.static_method(
            js_string!("test"),
            0,
            NativeFunction::from_fn_ptr(Self::static_test),
        );

        Ok(())
    }

    const LENGTH: usize = 0usize;

    const ATTRIBUTES: boa_engine::property::Attribute =
        boa_engine::property::Attribute::all();
}

pub struct TmpTestApi;

impl jstz_core::Api for TmpTestApi {
    fn init(self, context: &mut Context<'_>) {
        register_global_class::<TmpTestClass>(context)
            .expect("The `TmpTest` class shouldn't exist yet")
    }
}
