//! In order to run any JavaScript code in SpiderMonkey, an application must
//! have three key elements: a `Runtime``, a `Context`, and a global object.
//! This module implements a memory-safe wrapper for contexts.
//!
//! A `Runtime` contains the state for managing the garbage collector of
//! SpiderMonkey. All objects and contexts must be linked to a given runtime.
//! These objects cannot be shared across runtimes. Each thread must have a
//! unique `Runtime`.
//!
//! A `Context` contains the state for a virtual machine that executes and
//! manages JavaScript objects within a `Runtime`. It can compile and execute
//! scripts, get and set object properties, call JavaScript functions, convert
//! JavaScript data from one type to another, create objects, and so on.
//!
//! Global objects. Lastly, the global object contains all the classes,
//! functions, and variables that are available for JavaScript code to use.
//! Whenever JavaScript code does something like `window.open("http://jstz.dev/")`,
//! it is accessing a global property, in this case `window`.
//!
//! # Notes
//!
//! For more details, refer to the [ECMAScript Specification on Contexts](https://tc39.es/ecma262/#sec-global-environment-records).

use std::{ffi::c_void, marker::PhantomData, pin::Pin, ptr::NonNull};

use mozjs::{
    jsapi::{JSContext, JS_AddExtraGCRootsTracer, JS_RemoveExtraGCRootsTracer, JS},
    rust::Runtime,
};

use crate::{
    gc::{
        compartment::Compartment,
        ptr::AsRawPtr,
        root::{unsafe_ffi_trace_context_roots, Root, ShadowStack},
        Trace,
    },
    letroot,
    realm::Realm,
};

/// The context of a JavaScript runtime with a state `S`.
/// Ownership of a context represents the capability to manipulate data
/// managed by the engine.
pub struct Context<S> {
    raw_cx: NonNull<JSContext>,
    // SAFETY: This is only `Some` if the state `S` is `Entered<'a, C, S>`.
    // In this case, the old realm is guaranteed to be alive for at least as long as `'a`.
    old_realm: Option<*mut JS::Realm>,
    shadow_stack: Pin<Box<ShadowStack>>,
    marker: PhantomData<S>,
}

/// A context state for a JavaScript context owned by Rust.
pub struct Owned;

/// A context state for a JavaScript context provided by callbacks from JavaScript.
#[allow(dead_code)]
pub struct Callback;

/// A context state that has entered the compartment `C` with lifetime `'a`.
pub struct Entered<'a, C: Compartment, S> {
    marker: PhantomData<(&'a (), C, S)>,
}

// The following traits are 'marker' traits that are used to enforce
// type-level invariants on the context state.
pub trait CanAlloc {}
impl CanAlloc for Owned {}
impl CanAlloc for Callback {}
impl<'a, C: Compartment, S> CanAlloc for Entered<'a, C, S> {}

pub trait CanAccess {}
impl CanAccess for Owned {}
impl CanAccess for Callback {}
impl<'a, C: Compartment, S> CanAccess for Entered<'a, C, S> {}

pub trait InCompartment<C: Compartment> {}
impl<'a, C: Compartment, S> InCompartment<C> for Entered<'a, C, S> {}

fn new_shadow_stack(raw_cx: NonNull<JSContext>) -> Pin<Box<ShadowStack>> {
    // Initialize the GC roots for the context.
    let shadow_stack = Box::pin(ShadowStack::new());
    unsafe {
        JS_AddExtraGCRootsTracer(
            raw_cx.as_ptr(),
            Some(unsafe_ffi_trace_context_roots),
            &*shadow_stack as *const ShadowStack as *mut c_void,
        );
    }

    shadow_stack
}

impl Context<Owned> {
    pub fn from_runtime(rt: &Runtime) -> Self {
        // SAFETY: `rt.cx()` cannot be `NULL`.
        let raw_cx = unsafe { NonNull::new_unchecked(rt.cx()) };

        Self {
            raw_cx,
            old_realm: None,
            shadow_stack: new_shadow_stack(raw_cx),
            marker: PhantomData,
        }
    }
}

impl<S> Context<S> {
    /// Enter an existing realm
    pub fn enter_realm<'a, 'b, C: Compartment>(
        &'a mut self,
        realm: &Realm<'b, C>,
    ) -> Context<Entered<'a, C, S>>
    where
        S: CanAlloc + CanAccess,
        'a: 'b,
    {
        let old_realm = unsafe { JS::EnterRealm(self.as_raw_ptr(), realm.as_raw_ptr()) };

        Context {
            raw_cx: self.raw_cx,
            old_realm: Some(old_realm),
            shadow_stack: new_shadow_stack(self.raw_cx),
            marker: PhantomData,
        }
    }

    /// Enter a new realm
    pub fn new_realm<C>(&mut self, compartment: C) -> Option<Context<Entered<'_, C, S>>>
    where
        S: CanAlloc + CanAccess,
        C: Compartment,
    {
        letroot!(realm = Realm::new(compartment, self)?; [self]);

        Some(self.enter_realm(&realm))
    }

    /// Creates a new root
    pub fn root<T: Trace>(&self) -> Root<T> {
        Root::new(self.shadow_stack.as_ref())
    }
}

impl<S> Drop for Context<S> {
    fn drop(&mut self) {
        // Unroot everything in the current realm
        unsafe {
            JS_RemoveExtraGCRootsTracer(
                self.as_raw_ptr(),
                Some(unsafe_ffi_trace_context_roots),
                &*self.shadow_stack as *const ShadowStack as *mut c_void,
            );
        }

        // Leave the current realm
        if let Some(old_realm) = self.old_realm {
            unsafe {
                JS::LeaveRealm(self.as_raw_ptr(), old_realm);
            }
        }
    }
}

impl<S> AsRawPtr for Context<S> {
    type Ptr = *mut JSContext;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.raw_cx.as_ptr()
    }
}

#[cfg(test)]
mod test {
    use mozjs::{
        jsapi::JS,
        rust::{JSEngine, Runtime},
    };

    use crate::{alloc_compartment, gc::ptr::AsRawPtr};

    use super::Context;

    #[test]
    fn create_context_from_runtime() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let raw_cx = rt.cx();

        let cx = Context::from_runtime(&rt);

        assert_eq!(raw_cx, unsafe { cx.as_raw_ptr() })
    }

    #[test]
    fn entering_and_leaving_realm() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let cx = &mut Context::from_runtime(&rt);

        // Enter a new realm to evaluate the script in.
        alloc_compartment!(c1);
        let mut cx1 = cx.new_realm(c1).unwrap();
        let ptr = unsafe { cx1.as_raw_ptr() };
        let global1 = unsafe { JS::CurrentGlobalOrNull(cx1.as_raw_ptr()) };
        assert_eq!(global1, unsafe { JS::CurrentGlobalOrNull(ptr) });

        alloc_compartment!(c2);
        let cx2 = cx1.new_realm(c2).unwrap();
        let global2 = unsafe { JS::CurrentGlobalOrNull(cx2.as_raw_ptr()) };
        assert_ne!(global1, global2);
        assert_eq!(global2, unsafe { JS::CurrentGlobalOrNull(ptr) });

        drop(cx2);

        // Dropping the entered realm should restore the previous realm
        assert_eq!(global1, unsafe { JS::CurrentGlobalOrNull(ptr) });
    }
}
