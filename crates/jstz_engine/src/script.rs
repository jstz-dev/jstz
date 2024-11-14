//! This module provides an interface for JavaScript scripts in SpiderMonkey.
//! A `Script` encapsulates the parsed and compiled form of a JavaScript program,
//! enabling efficient execution and interaction with the JavaScript engine.
//!
//! ## Overview
//! - **Source Representation**: Links to the source text, including metadata
//!     like line numbers and column offsets for debugging / stack traces.
//! - **Bytecode**: Contains the compiled bytecode generated during parsing,
//!     optimized for SpiderMonkey's interpreter.
//!
//! For more details, refer to the [ECMAScript Specification on Scripts and Modules](https://tc39.es/ecma262/#sec-scripts).

use std::{marker::PhantomData, path::Path, ptr::NonNull};

use mozjs::{
    jsapi::{Compile1, JSScript, JS_ExecuteScript},
    jsval::{JSVal, UndefinedValue},
    rooted,
    rust::CompileOptionsWrapper,
};

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    AsRawPtr,
};

pub struct Script<'a, C: Compartment> {
    script: NonNull<JSScript>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Script<'a, C> {
    /// Compiles a script with a given filename and returns the compiled script.
    /// Returns `None` if the script could not be compiled.

    // TODO(https://linear.app/tezos/issue/JSTZ-210):
    // Add support for error handling / exceptions instead of using `Option`
    pub fn compile<S>(path: &Path, script: &str, cx: &'a mut Context<S>) -> Option<Self>
    where
        S: InCompartment<C> + CanAlloc,
    {
        let mut source = mozjs::rust::transform_str_to_source_text(script);
        let options = unsafe {
            CompileOptionsWrapper::new(cx.as_raw_ptr(), path.to_str().unwrap(), 1)
        };

        let script = unsafe { Compile1(cx.as_raw_ptr(), options.ptr, &mut source) };

        Some(Self {
            script: NonNull::new(script)?,
            marker: PhantomData,
        })
    }

    /// Evaluates a script and returns its return value

    // TODO(https://linear.app/tezos/issue/JSTZ-210):
    // Add support for error handling / exceptions instead of using `Option`
    // TODO(https://linear.app/tezos/issue/JSTZ-211):
    // TODO: `JSVal` is not safe, we should return a safe wrapper instead
    pub fn evaluate<'b, S>(&self, cx: &'b mut Context<S>) -> Option<JSVal>
    where
        S: InCompartment<C> + CanAlloc,
        'a: 'b,
    {
        // TODO(https://linear.app/tezos/issue/JSTZ-196):
        // Remove this once we have a proper way to root values
        rooted!(in(unsafe { cx.as_raw_ptr() }) let mut rval = UndefinedValue());
        rooted!(in(unsafe { cx.as_raw_ptr() }) let mut rooted_script = unsafe { self.as_raw_ptr() });

        if unsafe {
            JS_ExecuteScript(
                cx.as_raw_ptr(),
                rooted_script.handle_mut().into(),
                rval.handle_mut().into(),
            )
        } {
            Some(rval.get())
        } else {
            None
        }
    }
}

impl<'a, C: Compartment> AsRawPtr for Script<'a, C> {
    type Ptr = *mut JSScript;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.script.as_ptr()
    }
}

#[cfg(test)]
mod test {

    use std::path::PathBuf;

    use mozjs::rust::{JSEngine, Runtime};

    use crate::{compartment, context::Context, script::Script};

    #[test]
    fn test_compile_and_evaluate() {
        // Initialize the JS engine.
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let rt_cx = &mut Context::from_runtime(&rt);

        // Enter a new realm to evaluate the script in.
        let mut cx = rt_cx.new_realm().unwrap();

        // Some example source in a string.
        let filename = PathBuf::from("inline.js");
        let source: &'static str = "40 + 2";

        // Compile the script
        let script = Script::compile(&filename, source, &mut cx).unwrap();

        // TODO(https://linear.app/tezos/issue/JSTZ-196):
        // Remove once we have a proper way of rooting things.
        // The script is rooted in the context in `eval`, but this doesn't work due to lifetimes.
        // So we need to transmute it here.
        let rooted_script: Script<'_, compartment::Ref<'_>> =
            unsafe { std::mem::transmute(script) };

        // Evaluate the script
        let res = rooted_script.evaluate(&mut cx);

        assert!(res.is_some());

        let rval = res.unwrap();
        /* Should get a number back from the example source. */
        assert!(rval.is_int32());
        assert_eq!(rval.to_int32(), 42);
    }
}
