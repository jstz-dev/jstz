use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::jsval::{JSVal, UndefinedValue};

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    ffi::AsRawPtr,
    gc::{
        ptr::{GcPtr, Handle, HandleMut},
        Finalize, Prolong, Trace,
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

    pub(crate) unsafe fn from_jsval(jsval: JSVal) -> Self {
        Self {
            inner_ptr: GcPtr::pinned(jsval),
            marker: PhantomData,
        }
    }

    pub(crate) unsafe fn handle(&self) -> Handle<JSVal> {
        self.inner_ptr.handle()
    }

    pub(crate) unsafe fn handle_mut(&self) -> HandleMut<JSVal> {
        self.inner_ptr.handle_mut()
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

// pub enum JsVariant<'a, C> {
//     Null,
//     Undefined,
//     Integer32(u32),
//     Float64(f64),
//     Boolean(bool),
//     // Object(JsObject<'a, C>),
//     // String(JsString<'a, C>),
//     // Symbol(JsSymbol<'a, C>),
//     // BigInt(JsBigInt<'a, C>),
// }
