use boa_engine::{value::TryFromJs, Context, JsResult, JsValue};
use boa_gc::{custom_trace, Finalize, Trace};
use jstz_core::value::IntoJs;

/// A placeholder for types that have yet to be defined
#[derive(Debug)]
pub enum Todo {
    Todo,
}

impl Finalize for Todo {
    fn finalize(&self) {
        todo!()
    }
}

#[allow(unused_variables)]
unsafe impl Trace for Todo {
    custom_trace!(this, mark, todo!());
}

#[allow(unused_variables)]
impl IntoJs for Todo {
    fn into_js(self, context: &mut Context) -> JsValue {
        todo!()
    }
}

#[allow(unused_variables)]
impl TryFromJs for Todo {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        todo!()
    }
}
