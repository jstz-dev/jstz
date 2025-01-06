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

use std::{marker::PhantomData, path::Path, pin::Pin, sync::Arc};

use mozjs::{
    jsapi::{Compile1, Handle, JSScript, JS_ExecuteScript},
    jsval::{JSVal, UndefinedValue},
    rooted,
    rust::CompileOptionsWrapper,
};

use crate::{
    compartment::Compartment,
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{AsRawPtr, GcPtr},
        Finalize, Prolong, Trace,
    },
    letroot,
};

#[derive(Debug)]
pub struct Script<'a, C: Compartment> {
    script: Pin<Arc<GcPtr<*mut JSScript>>>,
    marker: PhantomData<(&'a (), C)>,
}

impl<'a, C: Compartment> Clone for Script<'a, C> {
    fn clone(&self) -> Self {
        Self {
            script: self.script.clone(),
            marker: PhantomData,
        }
    }
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

        if script.is_null() {
            return None;
        }

        Some(Self {
            script: GcPtr::pinned(script),
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

        if unsafe {
            JS_ExecuteScript(
                cx.as_raw_ptr(),
                Handle {
                    ptr: self.script.get_unsafe(),
                    _phantom_0: PhantomData,
                },
                rval.handle_mut().into(),
            )
        } {
            Some(rval.get())
        } else {
            None
        }
    }

    pub fn compile_and_evaluate<S>(
        path: &Path,
        src: &str,
        cx: &mut Context<S>,
    ) -> Option<JSVal>
    where
        S: InCompartment<C> + CanAlloc,
    {
        letroot!(script = Script::compile(path, src, cx)?; [cx]);

        script.evaluate(cx)
    }
}

impl<'a, C: Compartment> AsRawPtr for Script<'a, C> {
    type Ptr = *mut JSScript;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.script.get()
    }
}

impl<'a, C: Compartment> Finalize for Script<'a, C> {}

unsafe impl<'a, C: Compartment> Trace for Script<'a, C> {
    custom_trace!(this, mark, {
        mark(&this.script);
    });
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for Script<'b, C> {
    type Aged = Script<'a, C>;
}

#[cfg(test)]
mod test {

    use std::path::PathBuf;

    use mozjs::rust::{JSEngine, Runtime};

    use crate::{context::Context, letroot, script::Script};

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
        println!("Compiling script...");
        letroot!(script = Script::compile(&filename, source, &mut cx).unwrap(); [cx]);

        println!("Script, {:?}", script);

        // Evaluate the script
        let res = script.evaluate(&mut cx);

        assert!(res.is_some());

        let rval = res.unwrap();
        /* Should get a number back from the example source. */
        assert!(rval.is_int32());
        assert_eq!(rval.to_int32(), 42);
    }
}
