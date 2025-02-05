use std::marker::PhantomData;

use indoc::indoc;
use mozjs::jsval::{DoubleValue, Int32Value, JSVal, NullValue, UndefinedValue};

use crate::{
    bigint::JsBigInt,
    context::{CanAlloc, Context, InCompartment},
    gc::{
        ptr::{AsRawPtr, GcPtr},
        Compartment,
    },
    gcptr_wrapper,
    object::JsObject,
    string::JsString,
    symbol::JsSymbol,
};

gcptr_wrapper!(
    indoc! {"
        [`JsValue`] represents a generic JavaScript value. This is any valid ECMAScript value.
        
        More information:
         - [EMCAScript reference][spec]

        [spec]: https://tc39.es/ecma262/#sec-ecmascript-language-types
    "},
    JsValue,
    JSVal
);

impl<'a, C: Compartment> JsValue<'a, C> {
    pub fn is_undefined(&self) -> bool {
        unsafe { self.as_raw_ptr().is_undefined() }
    }

    pub fn is_null(&self) -> bool {
        unsafe { self.as_raw_ptr().is_null() }
    }

    pub fn is_i32(&self) -> bool {
        unsafe { self.as_raw_ptr().is_int32() }
    }

    pub fn is_f64(&self) -> bool {
        unsafe { self.as_raw_ptr().is_double() }
    }

    pub fn is_bool(&self) -> bool {
        unsafe { self.as_raw_ptr().is_boolean() }
    }

    pub fn is_object(&self) -> bool {
        unsafe { self.as_raw_ptr().is_object() }
    }

    pub fn is_string(&self) -> bool {
        unsafe { self.as_raw_ptr().is_string() }
    }

    pub fn is_symbol(&self) -> bool {
        unsafe { self.as_raw_ptr().is_symbol() }
    }

    pub fn is_bigint(&self) -> bool {
        unsafe { self.as_raw_ptr().is_bigint() }
    }

    pub fn null<S>(_: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(NullValue()) }
    }

    pub fn undefined<S>(_: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(UndefinedValue()) }
    }

    pub fn i32<S>(n: i32, _: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(Int32Value(n)) }
    }

    pub fn f64<S>(f: f64, _: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe { Self::from_raw(DoubleValue(f)) }
    }
}

pub enum JsVariant<'a, C: Compartment> {
    Null,
    Undefined,
    Integer32(u32),
    Float64(f64),
    Boolean(bool),
    Object(JsObject<'a, C>),
    String(JsString<'a, C>),
    Symbol(JsSymbol<'a, C>),
    BigInt(JsBigInt<'a, C>),
}
