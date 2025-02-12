use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::jsval::{JSVal, UndefinedValue};
mod conversions;

use crate::{
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{AsRawHandle, AsRawHandleMut, AsRawPtr, GcPtr, Handle, HandleMut},
        Compartment, Finalize, Prolong, Trace,
    },
};

pub struct JsValue<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<JSVal>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> AsRawPtr for JsValue<'a, C> {
    type Ptr = JSVal;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.inner_ptr.as_raw_ptr()
    }
}

impl<'a, C: Compartment> AsRawHandle for JsValue<'a, C> {
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr> {
        self.inner_ptr.as_raw_handle()
    }
}

impl<'a, C: Compartment> AsRawHandleMut for JsValue<'a, C> {
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr> {
        self.inner_ptr.as_raw_handle_mut()
    }
}

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

    #[allow(dead_code)]
    pub(crate) unsafe fn from_raw(jsval: JSVal) -> Self {
        Self {
            inner_ptr: GcPtr::pinned(jsval),
            marker: PhantomData,
        }
    }
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for JsValue<'b, C> {
    type Aged = JsValue<'a, C>;
}

impl<'a, C: Compartment> Finalize for JsValue<'a, C> {
    fn finalize(&self) {
        self.inner_ptr.finalize()
    }
}

unsafe impl<'a, C: Compartment> Trace for JsValue<'a, C> {
    custom_trace!(this, mark, {
        mark(&this.inner_ptr);
    });
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
