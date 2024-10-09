use boa_engine::{
    builtins::{array_buffer::ArrayBuffer, dataview::DataView, typed_array::TypedArray},
    object::builtins::{JsArrayBuffer, JsDataView, JsTypedArray},
    value::TryFromJs,
    Context, JsData, JsNativeError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

pub trait BufferSource {
    /// Gets a copy of the bytes held by the buffer source
    ///  
    /// https://webidl.spec.whatwg.org/#dfn-get-buffer-source-copy
    fn clone_data(&self, context: &mut Context) -> JsResult<Vec<u8>>;
}

impl BufferSource for JsArrayBuffer {
    fn clone_data(&self, _context: &mut Context) -> JsResult<Vec<u8>> {
        match self.data() {
            Some(buffer) => Ok(buffer.to_vec()),
            None => Err(JsNativeError::typ()
                .with_message("Buffer is detached")
                .into()),
        }
    }
}

impl BufferSource for JsDataView {
    fn clone_data(&self, context: &mut Context) -> JsResult<Vec<u8>> {
        let buffer: JsArrayBuffer = self.buffer(context)?.try_js_into(context)?;
        let offset = self.byte_offset(context)? as usize;
        let length = self.byte_length(context)? as usize;

        let buffer = buffer.clone_data(context)?;

        if offset + length > buffer.len() {
            return Err(JsNativeError::typ()
                .with_message("DataView byte range is out of bounds")
                .into());
        }

        Ok(buffer[offset..offset + length].to_vec())
    }
}

impl BufferSource for JsTypedArray {
    fn clone_data(&self, context: &mut Context) -> JsResult<Vec<u8>> {
        let buffer: JsArrayBuffer = self.buffer(context)?.try_js_into(context)?;
        let offset = self.byte_offset(context)?;
        let length = self.byte_length(context)?;

        let buffer = buffer.clone_data(context)?;

        if offset + length > buffer.len() {
            return Err(JsNativeError::typ()
                .with_message("TypedArray byte range is out of bounds")
                .into());
        }

        Ok(buffer[offset..offset + length].to_vec())
    }
}

#[derive(Trace, Finalize, JsData)]
pub enum JsArrayBufferView {
    TypedArray(JsTypedArray),
    DataView(JsDataView),
}

impl TryFromJs for JsArrayBufferView {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let Some(js_object) = value.as_object() else {
            return Err(JsNativeError::typ()
                .with_message("Expected `JsObject`")
                .into());
        };

        if js_object.is::<TypedArray>() {
            Ok(Self::TypedArray(value.try_js_into(context)?))
        } else if js_object.is::<DataView>() {
            Ok(Self::DataView(value.try_js_into(context)?))
        } else {
            Err(JsNativeError::typ()
                .with_message("The provided value is not of type `JsArrayBufferView`")
                .into())
        }
    }
}

impl BufferSource for JsArrayBufferView {
    fn clone_data(&self, context: &mut Context) -> JsResult<Vec<u8>> {
        match self {
            Self::TypedArray(typed_array) => typed_array.clone_data(context),
            Self::DataView(data_view) => data_view.clone_data(context),
        }
    }
}

#[derive(Trace, Finalize, JsData)]
pub enum JsBufferSource {
    ArrayBuffer(JsArrayBuffer),
    ArrayBufferView(JsArrayBufferView),
}

impl TryFromJs for JsBufferSource {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let Some(js_object) = value.as_object() else {
            return Err(JsNativeError::typ()
                .with_message("Expected `JsObject`")
                .into());
        };

        if js_object.is::<ArrayBuffer>() {
            Ok(Self::ArrayBuffer(value.try_js_into(context)?))
        } else if js_object.is::<TypedArray>() || js_object.is::<DataView>() {
            Ok(Self::ArrayBufferView(value.try_js_into(context)?))
        } else {
            Err(JsNativeError::typ()
                .with_message("The provided value is not of type `JsBufferSource`")
                .into())
        }
    }
}

impl BufferSource for JsBufferSource {
    fn clone_data(&self, context: &mut Context) -> JsResult<Vec<u8>> {
        match self {
            Self::ArrayBuffer(array_buffer) => array_buffer.clone_data(context),
            Self::ArrayBufferView(array_buffer_view) => {
                array_buffer_view.clone_data(context)
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
