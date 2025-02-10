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
        std::todo!()
    }
}

#[allow(unused_variables)]
unsafe impl Trace for Todo {
    custom_trace!(this, mark, std::todo!());
}

#[allow(unused_variables)]
impl IntoJs for Todo {
    fn into_js(self, context: &mut Context) -> JsValue {
        std::todo!()
    }
}

#[allow(unused_variables)]
impl TryFromJs for Todo {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        std::todo!()
    }
}

macro_rules! todo {
    ($msg:literal) => {
        return boa_engine::JsResult::Err(
            boa_engine::JsNativeError::error()
                .with_message(format!("todo: {}", String::from($msg)))
                .into(),
        )
    };
}
pub(crate) use todo;

#[cfg(test)]
mod tests {
    use boa_engine::{JsNativeError, JsResult};

    #[test]
    fn todo() {
        fn test() -> JsResult<()> {
            super::todo!("foobar");
        }
        assert_eq!(
            test().unwrap_err(),
            JsNativeError::error().with_message("todo: foobar").into()
        );
    }
}
