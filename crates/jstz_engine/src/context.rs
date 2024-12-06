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

use std::{marker::PhantomData, ptr::NonNull};

use mozjs::{jsapi::JSContext, rust::Runtime};

use crate::{compartment::Compartment, AsRawPtr};

/// The context of a JavaScript runtime with a state `S`.
/// Ownership of a context represents the capability to manipulate data
/// managed by the engine.
#[allow(dead_code)]
pub struct Context<S> {
    raw_cx: NonNull<JSContext>,
    marker: PhantomData<S>,
}

/// A context state for a JavaScript context owned by Rust.
pub struct Owned;

/// A context state for a JavaScript context provided by callbacks from JavaScript.
#[allow(dead_code)]
pub struct Callback;

/// A context state that has entered the compartment `C` with lifetime `'a`.
#[allow(dead_code)]
pub struct Entered<'a, C: Compartment, S> {
    marker: PhantomData<(&'a (), C, S)>,
}

// The following traits are 'marker' traits that are used to enforce
// type-level invariants on the context state.
#[allow(dead_code)]
pub trait CanAlloc {}
impl CanAlloc for Owned {}
impl CanAlloc for Callback {}
impl<'a, C: Compartment, S> CanAlloc for Entered<'a, C, S> {}

#[allow(dead_code)]
pub trait CanAccess {}
impl CanAccess for Owned {}
impl CanAccess for Callback {}
impl<'a, C: Compartment, S> CanAccess for Entered<'a, C, S> {}

#[allow(dead_code)]
pub trait InCompartment<C: Compartment> {}
impl<'a, C: Compartment, S> InCompartment<C> for Entered<'a, C, S> {}

impl Context<Owned> {
    pub fn from_runtime(rt: &Runtime) -> Self {
        // SAFETY: `rt.cx()` cannot be `NULL`.
        let raw_cx = unsafe { NonNull::new_unchecked(rt.cx()) };

        Self {
            raw_cx,
            marker: PhantomData,
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
    use mozjs::rust::{JSEngine, Runtime};

    use crate::AsRawPtr;

    use super::Context;

    #[test]
    fn create_context_from_runtime() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let raw_cx = rt.cx();

        let cx = Context::from_runtime(&rt);

        assert_eq!(raw_cx, unsafe { cx.as_raw_ptr() })
    }
}
