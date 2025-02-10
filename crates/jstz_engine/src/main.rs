#![allow(unused)]
use std::cell::RefCell;

use jstz_engine::compile_and_evaluate_script;
use mozjs::rust::JSEngine;
use tezos_smart_rollup::{entrypoint, host::Runtime};

thread_local! {
    /// Thread-local host context
    pub static JS_ENGINE: RefCell<Option<JSEngine>> = const { RefCell::new(None) };
}

#[cfg(feature = "native-kernel")]
#[entrypoint::main]
pub fn entry(_host: &mut impl Runtime) {
    JS_ENGINE.with(|js_engine| {
        if js_engine.borrow().is_none() {
            *js_engine.borrow_mut() = Some(JSEngine::init().unwrap())
        }
    });
    JS_ENGINE.with_borrow(|js_engine| {
        let handle = js_engine.as_ref().unwrap().handle();
        let source = "Math.random()";
        let rval = compile_and_evaluate_script(handle, source);
        assert!(rval.is_number());
        let number = rval.to_number();
        println!("{}", number)
    });
}

#[cfg(not(feature = "native-kernel"))]
pub fn main() {}
