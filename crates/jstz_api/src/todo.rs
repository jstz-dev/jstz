use boa_engine::value::TryFromJs;
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
    custom_trace!(this, todo!());
}

#[allow(unused_variables)]
impl IntoJs for Todo {
    fn into_js(
        self,
        context: &mut boa_engine::prelude::Context<'_>,
    ) -> boa_engine::prelude::JsValue {
        todo!()
    }
}

#[allow(unused_variables)]
impl TryFromJs for Todo {
    fn try_from_js(
        value: &boa_engine::prelude::JsValue,
        context: &mut boa_engine::prelude::Context<'_>,
    ) -> boa_engine::prelude::JsResult<Self> {
        todo!()
    }
}
