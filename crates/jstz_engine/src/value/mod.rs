use std::marker::PhantomData;

use indoc::indoc;
use mozjs::jsval::{JSVal, UndefinedValue};
mod conversions;

use crate::{
    context::{CanAlloc, Context, InCompartment},
    gc::{ptr::GcPtr, Compartment},
    gcptr_wrapper,
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
    pub fn undefined<S>(_: &'a mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        Self {
            inner_ptr: GcPtr::pinned(UndefinedValue()),
            marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use mozjs::rust::{JSEngine, Runtime};

    use crate::{alloc_compartment, context::Context};

    use super::JsValue;

    #[test]
    fn test_undefined() {
        // Initialize the JS engine.
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let rt_cx = &mut Context::from_runtime(&rt);
        alloc_compartment!(c);
        let mut cx = rt_cx.new_realm(c).unwrap();
        let val = JsValue::undefined(&mut cx);
        assert!(val.is_undefined())
    }
}
