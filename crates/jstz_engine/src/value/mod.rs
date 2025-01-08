use std::marker::PhantomData;

use indoc::indoc;
use mozjs::jsval::{JSVal, UndefinedValue};

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
