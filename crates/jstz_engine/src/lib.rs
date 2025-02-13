use std::path::PathBuf;

use context::Context;
use gc::ptr::AsRawPtr;
use mozjs::jsval::JSVal;
use mozjs::rust::{JSEngineHandle, Runtime};
use script::Script;

mod context;
pub mod gc;
mod realm;
mod script;
mod string;
mod value;

pub fn compile_and_evaluate_script(handle: JSEngineHandle, source: &str) -> JSVal {
    let rt = Runtime::new(handle);
    let rt_cx = &mut Context::from_runtime(&rt);

    // Enter a new realm to evaluate the script in.
    alloc_compartment!(c);
    let mut cx = rt_cx.new_realm(c).unwrap();

    // Some example source in a string.
    let filename = PathBuf::from("inline.js");

    // Compile the script
    letroot!(script = Script::compile(&filename, source, &mut cx).unwrap(); [cx]);

    // Evaluate the script
    let res = script.evaluate(&mut cx);
    assert!(res.is_some());

    let rval = res.unwrap();
    unsafe { rval.as_raw_ptr() }
}

#[cfg(test)]
mod test {

    use ::std::ptr;

    use mozjs::jsapi::*;
    use mozjs::jsval::UndefinedValue;
    use mozjs::rooted;
    use mozjs::rust::SIMPLE_GLOBAL_CLASS;
    use mozjs::rust::{JSEngine, RealmOptions, Runtime};

    #[macro_export]
    macro_rules! setup_cx {
        ($name: ident) => {
            let engine = mozjs::rust::JSEngine::init().unwrap();
            let rt = mozjs::rust::Runtime::new(engine.handle());
            let rt_cx = &mut $crate::context::Context::from_runtime(&rt);
            $crate::alloc_compartment!(c);
            let mut $name = rt_cx.new_realm(c).unwrap();
        };
    }

    #[test]
    fn test_eval() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());

        let options = RealmOptions::default();
        rooted!(in(rt.cx()) let global = unsafe {
            JS_NewGlobalObject(rt.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                               OnNewGlobalHookOption::FireOnNewGlobalHook,
                               &*options)
        });

        /* These should indicate source location for diagnostics. */
        let filename: &'static str = "inline.js";
        let lineno: u32 = 1;

        /*
         * The return value comes back here. If it could be a GC thing, you must add it to the
         * GC's "root set" with the rooted! macro.
         */
        rooted!(in(rt.cx()) let mut rval = UndefinedValue());

        /*
         * Some example source in a string. This is equivalent to JS_EvaluateScript in C++.
         */
        let source: &'static str = "40 + 2";

        let res = rt.evaluate_script(
            global.handle(),
            source,
            filename,
            lineno,
            rval.handle_mut(),
        );

        assert!(res.is_ok());
        /* Should get a number back from the example source. */
        assert!(rval.get().is_int32());
        assert_eq!(rval.get().to_int32(), 42);
    }
}
