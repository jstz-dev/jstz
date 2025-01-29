#![allow(dead_code)]
mod compartment;
mod context;
mod gc;
mod realm;
mod script;

pub(crate) trait AsRawPtr {
    type Ptr;

    /// Get the raw pointer to the underlying object.
    unsafe fn as_raw_ptr(&self) -> Self::Ptr;
}

#[cfg(test)]
mod test {

    use ::std::ptr;

    use mozjs::jsapi::*;
    use mozjs::jsval::UndefinedValue;
    use mozjs::rooted;
    use mozjs::rust::SIMPLE_GLOBAL_CLASS;
    use mozjs::rust::{JSEngine, RealmOptions, Runtime};

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
