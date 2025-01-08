use std::{marker::PhantomData, pin::Pin, sync::Arc};

use bitflags::bitflags;
use mozjs::jsapi::{
    jsid, JSITER_FORAWAITOF, JSITER_HIDDEN, JSITER_OWNONLY, JSITER_PRIVATE,
    JSITER_SYMBOLS, JSITER_SYMBOLSONLY, JSPROP_ENUMERATE, JSPROP_PERMANENT,
    JSPROP_READONLY, JSPROP_RESOLVING,
};

use crate::{
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{AsRawHandle, AsRawHandleMut, AsRawPtr, GcPtr, Handle, HandleMut},
        Compartment, Finalize, Prolong, Trace,
    },
};

pub struct PropertyKey<'a, C: Compartment> {
    inner_ptr: Pin<Arc<GcPtr<jsid>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> PropertyKey<'a, C> {
    pub(crate) fn from_raw(raw: jsid) -> Self {
        Self {
            inner_ptr: GcPtr::pinned(raw),
            marker: PhantomData,
        }
    }
}

impl<'a, C: Compartment> Finalize for PropertyKey<'a, C> {}

unsafe impl<'a, C: Compartment> Trace for PropertyKey<'a, C> {
    custom_trace!(this, mark, mark(&this.inner_ptr));
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for PropertyKey<'b, C> {
    type Aged = PropertyKey<'a, C>;
}

impl<'a, C: Compartment> AsRawPtr for PropertyKey<'a, C> {
    type Ptr = jsid;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.inner_ptr.as_raw_ptr()
    }
}

impl<'a, C: Compartment> AsRawHandle for PropertyKey<'a, C> {
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr> {
        self.inner_ptr.as_raw_handle()
    }
}

impl<'a, C: Compartment> AsRawHandleMut for PropertyKey<'a, C> {
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr> {
        self.inner_ptr.as_raw_handle_mut()
    }
}

pub trait IntoPropertyKey {
    /// Converts `self` into a new [`PropertyKey`].
    /// Returns [`None`] when conversion fails.
    fn into_key<C: Compartment, S>(self, cx: &mut Context<S>) -> Option<PropertyKey<C>>
    where
        S: InCompartment<C> + CanAlloc;
}

bitflags! {
    /// Represents the flags of properties on an `[JsObject]`
    #[derive(Clone, Copy, Debug)]
    pub struct PropertyFlags: u16 {
        /// Allows enumeration through `Object.keys()`, `for...in` and other functions.
        /// See [Enumerability of Properties](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Enumerability_and_ownership_of_properties#traversing_object_properties).
        const ENUMERATE = JSPROP_ENUMERATE as u16;

        /// Prevents reassignment of the property.
        const READ_ONLY = JSPROP_READONLY as u16;

        /// Prevents deletion and attribute modification of the property.
        const PERMANENT = JSPROP_PERMANENT as u16;
        const RESOLVING = JSPROP_RESOLVING as u16;

        const CONSTANT = PropertyFlags::READ_ONLY.bits() | PropertyFlags::PERMANENT.bits();
        const CONSTANT_ENUMERATED = PropertyFlags::CONSTANT.bits() | PropertyFlags::ENUMERATE.bits();
    }
}

bitflags! {
    /// Represents the flags when iterating over an [Object](crate::Object).
    #[derive(Clone, Copy, Debug, Default)]
    pub struct PropertyIteratorFlags: u32 {
        /// Allows iterating over private properties.
        const PRIVATE = JSITER_PRIVATE;
        /// Disallows iterating over inherited properties.
        const OWN_ONLY = JSITER_OWNONLY;
        /// Allows iteration over non-enumerable properties.
        const HIDDEN = JSITER_HIDDEN;
        /// Allows iteration over symbol keys.
        const SYMBOLS = JSITER_SYMBOLS;
        /// Disallows iteration over string keys.
        const SYMBOLS_ONLY = JSITER_SYMBOLSONLY;
        /// Iteration over async iterable objects and async generators.
        const FOR_AWAIT_OF = JSITER_FORAWAITOF;
    }
}
