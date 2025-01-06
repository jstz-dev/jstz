use crate::{compartment::Compartment, gc::ptr::AsRawPtr};

use super::JsValue;

impl<'a, C: Compartment> JsValue<'a, C> {
    pub fn is_undefined(&self) -> bool {
        unsafe { self.as_raw_ptr().is_undefined() }
    }
}

#[cfg(test)]
mod test {
    use mozjs::rust::{JSEngine, Runtime};

    use crate::{context::Context, value::JsValue};

    #[test]
    fn test_is_undefined() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let rt_cx = &mut Context::from_runtime(&rt);
        let mut cx = rt_cx.new_realm().unwrap();
        let val = JsValue::undefined(&mut cx);
        assert!(val.is_undefined());
    }
}
