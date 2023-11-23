use std::ops::Deref;

use boa_engine::{
    object::builtins::{JsDataView, JsTypedArray},
    value::TryFromJs,
    Context, JsError, JsObject, JsResult, JsValue,
};
use jstz_core::value::IntoJs;

pub type Chunk = JsValue;

pub type ChunkOrUndefined = JsValue; // TODO replace by JsOptional<JsValue>

// TODO valid def?
pub enum JsArrayBufferView {
    TypedArray(JsTypedArray),
    DataView(JsDataView),
}

pub type UnrestrictedDouble = f64;
pub type Any = JsValue;

pub type UnsignedLongLong = i64;

pub type PositiveInteger = UnsignedLongLong;

pub type Number = f64;
