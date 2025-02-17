use indoc::indoc;
use mozjs::jsval::{DoubleValue, Int32Value, NullValue};
use mozjs::jsval::{JSVal, UndefinedValue};

use crate::gc::ptr::AsRawPtr;
use crate::{
    context::{CanAlloc, Context, InCompartment},
    gc::compartment::Compartment,
    gcptr_wrapper,
};

mod conversions;
#[allow(unused_imports)]
pub use conversions::{try_from_js::TryFromJs, try_into_js::TryIntoJs};

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

    pub fn is_double(&self) -> bool {
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

#[cfg(test)]
mod test {

    use crate::{setup_cx, value::TryIntoJs};

    use super::JsValue;

    #[test]
    fn test_is_undefined() {
        setup_cx!(cx);
        let val = JsValue::undefined(&mut cx);
        assert!(val.is_undefined())
    }

    #[test]
    fn test_is_null() {
        setup_cx!(cx);
        let val = JsValue::null(&mut cx);
        assert!(val.is_null())
    }

    #[test]
    fn test_is_string() {
        setup_cx!(cx);
        let value = "hello".try_into_js(&mut cx).unwrap();
        assert!(value.is_string())
    }

    #[test]
    fn test_is_i32() {
        setup_cx!(cx);
        let value = JsValue::i32(42, &mut cx);
        assert!(value.is_i32())
    }

    #[test]
    fn test_is_double() {
        setup_cx!(cx);
        let value = JsValue::f64(42.0, &mut cx);
        assert!(value.is_double())
    }
}
