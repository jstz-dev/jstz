//! This module provides the interface for JavaScript Realms in SpiderMonkey.
//! A realm represents a distinct execution environment for JavaScript code,
//! encapsulating global objects, intrinsic objects, and a separate environment
//! for executing scripts and modules.
//!
//! Realms are fundamental to the JavaScript specification and enable features such as:
//! - **Isolation**: Code execution in one realm cannot directly affect another realm's
//!   execution environment, making realms ideal for sandboxing.
//! - **Multiple Global Environments**: Each realm has its own `global` object and associated
//!   built-ins like `Array`, `Object`, and `Function`.
//!
//! # Key Concepts
//!
//! - **Global Object**: Each realm contains its unique global object, which is the root
//!   of the scope chain for all scripts executed within that realm.
//! - **Intrinsics**: Realms maintain their own set of intrinsic objects, such as
//!   `Object.prototype` and `Array.prototype`, ensuring isolation at the object level.
//! - **Compartments**: Realms exist within compartments, which group related
//!   realms
//!
//! # Notes
//!
//! For more details, refer to the [ECMAScript Specification on Realms](https://tc39.es/ecma262/#sec-code-realms).

use std::{marker::PhantomData, pin::Pin, sync::Arc};

use mozjs::{
    jsapi::{JSObject, JS_NewGlobalObject, OnNewGlobalHookOption, JS},
    rust::{RealmOptions, SIMPLE_GLOBAL_CLASS},
};

use crate::{
    context::{CanAccess, CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        compartment::Compartment,
        ptr::{AsRawPtr, GcPtr},
        Finalize, Prolong, Trace,
    },
};

/// A JavaScript realm with lifetime of at least `'a` allocated in compartment `C`.
/// A realm is a global object.
#[derive(Debug)]
pub struct Realm<'a, C: Compartment> {
    global_object: Pin<Arc<GcPtr<*mut JSObject>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Clone for Realm<'a, C> {
    fn clone(&self) -> Self {
        Self {
            global_object: self.global_object.clone(),
            marker: PhantomData,
        }
    }
}

impl<'a, C: Compartment> Realm<'a, C> {
    // Creates a new Realm in compartment C in the context of cx.
    // The unused [_compartment] argument is potentially a witness for the lifetime
    // of Self in C because C can be Ref<'b>. If so, then the returned Realm will be
    // bounded by two lifetimes - the lifetime of cx ('a) and of _compartment ('b)
    pub fn new<S>(_compartment: C, cx: &'a mut Context<S>) -> Option<Self>
    where
        S: CanAlloc + CanAccess,
    {
        // NOTE: [RealmOptions::default()] enables the creation of a new compartment for this
        //       realm. If we want to use an existing compartment, this will need to be altered.
        let mut realm_options = RealmOptions::default();
        realm_options.creationOptions_.sharedMemoryAndAtomics_ = true;
        realm_options
            .creationOptions_
            .defineSharedArrayBufferConstructor_ = true;

        let global_object = unsafe {
            JS_NewGlobalObject(
                cx.as_raw_ptr(),
                &SIMPLE_GLOBAL_CLASS,
                std::ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook,
                &*realm_options,
            )
        };

        if global_object.is_null() {
            return None;
        }

        Some(Self {
            global_object: GcPtr::pinned(global_object),
            marker: PhantomData,
        })
    }
}

impl<'a, C: Compartment> Realm<'a, C> {
    pub fn from_context<S>(cx: &'a mut Context<S>) -> Option<Self>
    where
        S: InCompartment<C>,
    {
        let global_object = unsafe { JS::CurrentGlobalOrNull(cx.as_raw_ptr()) };

        if global_object.is_null() {
            return None;
        }

        Some(Self {
            global_object: GcPtr::pinned(global_object),
            marker: PhantomData,
        })
    }
}

impl<'a, C: Compartment> AsRawPtr for Realm<'a, C> {
    type Ptr = *mut JSObject;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.global_object.get()
    }
}

impl<'a, C: Compartment> Finalize for Realm<'a, C> {}

unsafe impl<'a, C: Compartment> Trace for Realm<'a, C> {
    custom_trace!(this, mark, {
        mark(&this.global_object);
    });
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for Realm<'b, C> {
    type Aged = Realm<'a, C>;
}
