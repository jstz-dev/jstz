use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::jsapi::jsid;

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{GcPtr, Handle, HandleMut},
        Finalize, Prolong, Trace,
    },
};

pub struct PropertyKey<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<jsid>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Finalize for PropertyKey<'a, C> {}

unsafe impl<'a, C: Compartment> Trace for PropertyKey<'a, C> {
    custom_trace!(this, mark, mark(&this.inner_ptr));
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for PropertyKey<'b, C> {
    type Aged = PropertyKey<'a, C>;
}

impl<'a, C: Compartment> PropertyKey<'a, C> {
    pub(crate) unsafe fn handle(&self) -> Handle<jsid> {
        self.inner_ptr.handle()
    }

    pub(crate) unsafe fn handle_mut(&self) -> HandleMut<jsid> {
        self.inner_ptr.handle_mut()
    }
}

pub trait IntoPropertyKey<'a, C: Compartment> {
    /// Converts `self` into a new [`PropertyKey`].
    /// Returns [`None`] when conversion fails.
    fn into_key<'cx, S>(self, cx: &'cx mut Context<S>) -> Option<PropertyKey<'cx, C>>
    where
        S: InCompartment<C> + CanAlloc,
        'a: 'cx;
}

// pub enum OwnedKey<'a, C: Compartment> {
//     Index(u32),
//     String(JsString<'a, C>),
//     Symbol(JsSymbol<'a, C>),
// }
