use std::ops::Deref;

use boa_engine::{
    object::{
        builtins::{JsArrayBuffer, JsDataView, JsTypedArray},
        Object,
    },
    value::TryFromJs,
    Context, JsError, JsNativeError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, GcRef, GcRefMut, Trace};

pub trait ArrayBufferLike: Trace + Finalize + Sized {
    fn to_array_buffer_data(
        &self,
        context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData>;
}

impl ArrayBufferLike for JsArrayBuffer {
    fn to_array_buffer_data(
        &self,
        _context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData> {
        Ok(JsArrayBufferData {
            inner: self.deref().clone(),
        })
    }
}

impl ArrayBufferLike for JsTypedArray {
    fn to_array_buffer_data(
        &self,
        _context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData> {
        let this = self.deref().borrow();

        let integer_indexed = this.as_typed_array().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("The provided value is not of type `JsTypedArray`"),
            )
        })?;

        let array_buffer = integer_indexed.viewed_array_buffer().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ().with_message("The typed array has no array buffer"),
            )
        })?;

        Ok(JsArrayBufferData {
            inner: array_buffer.clone(),
        })
    }
}

impl ArrayBufferLike for JsDataView {
    fn to_array_buffer_data(
        &self,
        context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData> {
        let JsValue::Object(array_buffer) = self.buffer(context)? else {
            return Err(JsNativeError::typ()
                .with_message("The provided value is not of type `JsObject`")
                .into());
        };

        Ok(JsArrayBufferData {
            inner: array_buffer,
        })
    }
}

pub struct JsArrayBufferData {
    // INVARIANT: The `JsObject` is an `ArrayBuffer`
    inner: JsObject,
}

impl JsArrayBufferData {
    pub fn from_array_buffer_like<T: ArrayBufferLike>(
        buffer_source: &T,
        context: &mut Context<'_>,
    ) -> JsResult<Self> {
        buffer_source.to_array_buffer_data(context)
    }

    pub fn as_slice(&self) -> Option<GcRef<'_, [u8]>> {
        GcRef::try_map(self.inner.borrow(), |array_buffer| {
            let array_buffer = array_buffer
                .as_array_buffer()
                .expect("The provided value is not of type `JsArrayBuffer`");

            array_buffer.array_buffer_data.as_deref()
        })
    }

    pub fn as_slice_mut(&self) -> Option<GcRefMut<'_, Object, [u8]>> {
        GcRefMut::try_map(self.inner.borrow_mut(), |array_buffer| {
            let array_buffer = array_buffer
                .as_array_buffer_mut()
                .expect("The provided value is not of type `JsArrayBuffer`");

            array_buffer.array_buffer_data.as_deref_mut()
        })
    }
}

#[derive(Trace, Finalize)]
pub enum JsArrayBufferView {
    TypedArray(JsTypedArray),
    DataView(JsDataView),
}

impl TryFromJs for JsArrayBufferView {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let Some(js_object) = value.as_object() else {
            return Err(JsNativeError::typ()
                .with_message("Expected `JsObject`")
                .into());
        };

        if js_object.is_typed_array() {
            Ok(Self::TypedArray(value.try_js_into(context)?))
        } else if js_object.is_data_view() {
            Ok(Self::DataView(value.try_js_into(context)?))
        } else {
            Err(JsNativeError::typ()
                .with_message("The provided value is not of type `JsArrayBufferView`")
                .into())
        }
    }
}

impl ArrayBufferLike for JsArrayBufferView {
    fn to_array_buffer_data(
        &self,
        context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData> {
        match self {
            Self::TypedArray(typed_array) => typed_array.to_array_buffer_data(context),
            Self::DataView(data_view) => data_view.to_array_buffer_data(context),
        }
    }
}

#[derive(Trace, Finalize)]
pub enum JsBufferSource {
    ArrayBuffer(JsArrayBuffer),
    ArrayBufferView(JsArrayBufferView),
}

impl TryFromJs for JsBufferSource {
    fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
        let Some(js_object) = value.as_object() else {
            return Err(JsNativeError::typ()
                .with_message("Expected `JsObject`")
                .into());
        };

        if js_object.is_array_buffer() {
            Ok(Self::ArrayBuffer(value.try_js_into(context)?))
        } else if js_object.is_typed_array() || js_object.is_data_view() {
            Ok(Self::ArrayBufferView(value.try_js_into(context)?))
        } else {
            Err(JsNativeError::typ()
                .with_message("The provided value is not of type `JsBufferSource`")
                .into())
        }
    }
}

impl ArrayBufferLike for JsBufferSource {
    fn to_array_buffer_data(
        &self,
        context: &mut Context<'_>,
    ) -> JsResult<JsArrayBufferData> {
        match self {
            Self::ArrayBuffer(array_buffer) => array_buffer.to_array_buffer_data(context),
            Self::ArrayBufferView(array_buffer_view) => {
                array_buffer_view.to_array_buffer_data(context)
            }
        }
    }
}

// https://webidl.spec.whatwg.org/#idl-types

pub type Any = JsValue;
pub type Bytes = i8;
pub type Octet = u8;
pub type Short = i16;
pub type UnsignedShort = u16;
pub type Long = i32;
pub type UnsignedLong = u32;
pub type LongLong = i64;
pub type UnsignedLongLong = u64;
pub type UnrestrictedFloat = f32;
pub type UnrestrictedDouble = f64;

pub type PositiveInteger = UnsignedLongLong;
pub type Number = f64;
