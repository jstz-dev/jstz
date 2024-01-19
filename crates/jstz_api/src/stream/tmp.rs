//! Temporary definitions to allow compiling before defining all types

use boa_engine::{
    js_string, property::PropertyKey, Context, JsObject, JsResult, JsValue,
};

use crate::todo::Todo;

pub type ReadableStreamDefaultController = Todo;
pub type ReadableByteStreamController = Todo;

// TODO check that this function works as intended in all cases,
// and move it either to a new derive macro for TryFromJs, or to JsObject
pub fn get_jsobject_property(
    obj: &JsObject,
    name: &str,
    context: &mut Context<'_>,
) -> JsResult<JsValue> {
    let key = PropertyKey::from(js_string!(name));
    let has_prop = obj.has_property(key.clone(), context)?;
    if has_prop {
        obj.get(key, context)
    } else {
        Ok(JsValue::Undefined)
    }
}
