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

use std::{marker::PhantomData, ptr::NonNull};

use mozjs::{
    jsapi::{JSObject, JS_NewGlobalObject, OnNewGlobalHookOption, JS},
    rust::{RealmOptions, SIMPLE_GLOBAL_CLASS},
};

use crate::{
    compartment::{self, Compartment},
    context::{CanAccess, CanAlloc, Context, InCompartment},
    AsRawPtr,
};

/// A JavaScript realm with lifetime of at least `'a` allocated in compartment `C`.
/// A realm is a global object.
pub struct Realm<'a, C: Compartment> {
    global_object: NonNull<JSObject>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Copy for Realm<'a, C> {}

impl<'a, C: Compartment> Clone for Realm<'a, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a> Realm<'a, compartment::Ref<'a>> {
    pub fn new<S>(cx: &'a mut Context<S>) -> Option<Self>
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

        Some(Self {
            global_object: NonNull::new(global_object)?,
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

        Some(Self {
            global_object: NonNull::new(global_object)?,
            marker: PhantomData,
        })
    }
}

impl<'a, C: Compartment> AsRawPtr for Realm<'a, C> {
    type Ptr = *mut JSObject;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.global_object.as_ptr()
    }
}
