//! `jstz`'s implementation of JavaScript's `Blob` API.
//!
//! More information:
//!  - [MDN documentation][mdn]
//!  - [W3C `File` specification][spec]
//!  - [WHATWG `Infra` specification][infra-spec]
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Blob
//! [spec]: https://w3c.github.io/FileAPI/
//! [infra-spec]: https://infra.spec.whatwg.org/

use std::cmp::{max, min};

use boa_engine::{
    builtins::{array_buffer::ArrayBuffer, dataview::DataView, typed_array::TypedArray},
    js_string,
    object::{
        builtins::{JsArray, JsArrayBuffer, JsPromise},
        ErasedObject,
    },
    property::Attribute,
    value::TryFromJs,
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsString, JsValue,
    NativeFunction,
};
use boa_gc::{Finalize, GcRefMut, Trace};
use jstz_core::{
    accessor,
    native::{
        register_global_class, Accessor, ClassBuilder, JsNativeObject, NativeClass,
    },
    value::IntoJs,
};

use crate::idl::{BufferSource, JsBufferSource};

#[derive(Trace, Finalize, JsData, Clone)]
pub struct Blob {
    // TODO: Use https://docs.rs/bytes/1.5.0/bytes/
    bytes: Vec<u8>,
    size: u64,
    type_: String,
}

// https://infra.spec.whatwg.org/#collect-a-sequence-of-code-points
fn collect_code_points(mut position: &[u16]) -> (Vec<u16>, &[u16]) {
    // 1. Let result be the empty string.
    let mut result: Vec<u16> = vec![];
    // 2. While position doesn’t point past the end of input and the code point at position within
    //    input meets the condition condition:
    while !position.is_empty() && (position[0] == 0x000A || position[0] == 0x000D) {
        // 1. Append that code point to the end of result.
        result.push(position[0]);
        // 2. Advance position by 1.
        position = &position[1..];
    }
    // 3. Return result.
    (result, position)
}

// https://w3c.github.io/FileAPI/#convert-line-endings-to-native
fn convert_line_endings_to_native(s: Vec<u16>) -> Vec<u16> {
    // 1. Let native line ending be be the code point U+000A LF.
    let native_line_ending: u16 = 0x000A;
    // 3. Set result to the empty string.
    let mut result: Vec<u16> = vec![];
    // 4. Let position be a position variable for s, initially pointing at the start of s.
    let position = s;
    // 5. Let token be the result of collecting a sequence of code points that are not equal
    //    to U+000A LF or U+000D CR from s given position.
    let (mut token, mut position) = collect_code_points(&position);
    // 6. Append token to result.
    result.append(&mut token);
    // 7. While position is not past the end of s:
    while !position.is_empty() {
        // 1. If the code point at position within s equals U+000D CR:
        if position[0] == 0x000D {
            // 1. Append native line ending to result.
            result.push(native_line_ending);
            // 2. Advance position by 1.
            position = &position[1..];
            // 3. If position is not past the end of s and the code point at position within s
            //    equals U+000A LF advance position by 1.
            if !position.is_empty() && position[0] == 0x000A {
                position = &position[1..];
            }
        }
        // 2. Otherwise if the code point at position within s equals U+000A LF, advance
        //    position by 1 and append native line ending to result.
        else if position[0] == 0x000A {
            position = &position[1..];
            result.push(native_line_ending);
        }
        // 3. Let token be the result of collecting a sequence of code points that are not
        //    equal to U+000A LF or U+000D CR from s given position.
        (token, position) = collect_code_points(position);
        // 4. Append token to result.
        result.append(&mut token);
    }
    // 8. Return result.
    result
}

fn bytes_to_string(bytes: &[u8]) -> JsResult<String> {
    String::from_utf8(bytes.to_owned()).map_err(|_| {
        JsError::from_native(
            JsNativeError::typ().with_message("Failed to convert bytes into utf8 text"),
        )
    })
}

fn normalize_type(t: &str) -> String {
    // 1. If t contains any characters outside the range of U+0020 to U+007E, then set t
    //    to the empty string and return from these substeps.
    for c in t.chars() {
        match c {
            '\u{0020}'..='\u{007E}' => (),
            _ => return String::new(),
        }
    }
    // 2. Convert every character in relativeContentType to ASCII lowercase.
    t.to_ascii_lowercase()
}

fn utf8_encoding(s: &[u16]) -> String {
    char::decode_utf16(s.iter().copied())
        .map(|r| match r {
            Ok(c) => String::from(c),
            Err(_) => String::from("\u{FFFD}"),
        })
        .collect()
}

// https://w3c.github.io/FileAPI/#process-blob-parts
fn process_blob_parts(
    blob_parts: BlobParts,
    options: &BlobPropertyBag,
    context: &mut Context,
) -> JsResult<Vec<u8>> {
    let BlobParts(blob_parts) = blob_parts;
    // 1. Let bytes be an empty sequence of bytes.
    let mut bytes: Vec<u8> = vec![];
    // 2. For each element in parts:
    for element in blob_parts.iter() {
        match element {
            // 1. If element is a USVString, run the following substeps:
            BlobPart::String(string) => {
                // 1. Let s be element.
                let s = string.to_vec();
                // 2. If the endings member of options is "native", set s to the result
                //    of converting line endings to native of element.
                let s = match options {
                    BlobPropertyBag {
                        endings: Some(Endings::Native),
                        type_: _,
                    } => convert_line_endings_to_native(s),
                    _ => s.to_vec(),
                };
                // 3. Append the result of UTF-8 encoding s to bytes.
                bytes.append(&mut utf8_encoding(s.as_slice()).into())
            }
            // 2. If element is a BufferSource, get a copy of the bytes held by the
            //    buffer source, and append those bytes to bytes.
            BlobPart::BufferSource(buffer_source) => {
                let mut b = buffer_source.clone_data(context)?;
                bytes.append(&mut b)
            }
            // 3. If element is a Blob, append the bytes it represents to bytes.
            BlobPart::Blob(blob) => bytes.append(&mut blob.bytes.clone()),
        }
    }
    // 3. Return bytes.
    Ok(bytes)
}

impl Blob {
    // https://w3c.github.io/FileAPI/#constructorBlob
    pub fn new(
        blob_parts: Option<BlobParts>,
        options: Option<BlobPropertyBag>,
        context: &mut Context,
    ) -> JsResult<Self> {
        // 1. If invoked with zero parameters, return a new Blob object consisting of
        //    0 bytes, with size set to 0, and with type set to the empty string.
        let (blob_parts, options) = match (blob_parts, options) {
            (None, None) => {
                return Ok(Self {
                    bytes: vec![],
                    type_: String::new(),
                    size: 0,
                })
            }
            (None, Some(_)) => {
                return Err(JsError::from_native(
                    JsNativeError::typ()
                        .with_message("Expected `blobParts` argument to be present"),
                ))
            }
            (Some(blob_parts), None) => (blob_parts, BlobPropertyBag::default()),
            (Some(blob_parts), Some(options)) => (blob_parts, options),
        };
        // 2. Let bytes be the result of processing blob parts given blobParts and options.
        let bytes = process_blob_parts(blob_parts, &options, context)?;
        let mut t = String::new();
        // 3. If the type member of the options argument is not the empty string, run the
        //    following sub-steps (see normalize_type)
        if let Some(s) = options.type_ {
            if !s.is_empty() {
                t = normalize_type(s.as_str());
            }
        }
        // 4. Return a Blob object referring to bytes as its associated byte sequence,
        //    with its size set to the length of bytes, and its type set to the value of
        //    t from the substeps above.
        let type_ = t;
        let size = bytes.len() as u64;
        Ok(Self { bytes, type_, size })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn type_(&self) -> String {
        self.type_.clone()
    }

    pub fn text(&mut self, context: &mut Context) -> JsResult<JsPromise> {
        let s = js_string!(bytes_to_string(&self.bytes)?);
        Ok(JsPromise::resolve(s, context))
    }

    pub fn array_buffer(&mut self, context: &mut Context) -> JsResult<JsPromise> {
        let b = &self.bytes;
        Ok(JsPromise::resolve(
            JsArrayBuffer::from_byte_block(b.to_vec(), context)?,
            context,
        ))
    }

    // https://w3c.github.io/FileAPI/#slice-blob
    pub fn slice(
        &self,
        start: Option<i64>,
        end: Option<i64>,
        content_type: Option<String>,
    ) -> Blob {
        // 1. Let originalSize be blob’s size.
        let original_size = self.size as i64;
        // 2. The start parameter, if non-null, is a value for the start point of a slice blob call,
        //    and must be treated as a byte-order position, with the zeroth position representing
        //    the first byte. User agents must normalize start according to the following:
        //     a. If start is null, let relativeStart be 0.
        let relative_start = start.map_or(0, |start| {
            // b. If start is negative, let relativeStart be max((originalSize + start), 0).
            if start < 0 {
                max(original_size + start, 0)
            }
            // c. Otherwise, let relativeStart be min(start, originalSize).
            else {
                min(start, original_size)
            }
        });
        // 3. The end parameter, if non-null. is a value for the end point of a slice blob call.
        //    User agents must normalize end according to the following:
        //     a. If end is null, let relativeEnd be originalSize.
        let relative_end = end.map_or(original_size, |end| {
            // b. If end is negative, let relativeEnd be max((originalSize + end), 0).
            if end < 0 {
                max(original_size + end, 0)
            }
            // c. Otherwise, let relativeEnd be min(end, originalSize).
            else {
                min(end, original_size)
            }
        });
        // 4. The contentType parameter, if non-null, is used to set the ASCII-encoded string in
        //    lower case representing the media type of the Blob.
        //    User agents must normalize contentType according to the following:
        let relative_content_type = match content_type {
            // a. If contentType is null, let relativeContentType be set to the empty string.
            None => String::new(),
            // b. Otherwise, let relativeContentType be set to contentType and run the substeps
            //    (see normalize_type)
            Some(content_type) => normalize_type(&content_type),
        };
        // 5. Let span be max((relativeEnd - relativeStart), 0).
        let span = max(relative_end - relative_start, 0) as usize;
        let relative_start = relative_start as usize;
        // 6. Return a new Blob object S with the following characteristics:
        //    a. S refers to span consecutive bytes from blob’s associated byte sequence,
        //       beginning with the byte at byte-order position relativeStart.
        let bytes = self.bytes[relative_start..relative_start + span].to_vec();
        //    b. S.size = span.
        let size = span as u64;
        //    c. S.type = relativeContentType.
        let type_ = relative_content_type;
        Blob { bytes, size, type_ }
    }
}

impl Blob {
    pub fn try_from_js(value: &JsValue) -> JsResult<GcRefMut<'_, ErasedObject, Self>> {
        value
            .as_object()
            .and_then(|obj| obj.downcast_mut::<Self>())
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("Failed to convert js value into rust type `Blob`")
                    .into()
            })
    }
}

pub enum BlobPart {
    BufferSource(JsBufferSource),
    Blob(Blob),
    String(JsString),
}

impl TryFromJs for BlobPart {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        if value.is_string() {
            let string: String = value.try_js_into(context)?;
            return Ok(Self::String(js_string!(string)));
        }
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected object"))
        })?;
        if obj.is::<ArrayBuffer>() || obj.is::<TypedArray>() || obj.is::<DataView>() {
            return Ok(Self::BufferSource(JsBufferSource::try_from_js(
                value, context,
            )?));
        }

        let blob = Blob::try_from_js(value)?;
        Ok(Self::Blob(blob.clone()))
    }
}

pub struct BlobParts(Vec<BlobPart>);

impl TryFromJs for BlobParts {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let mut vec: Vec<BlobPart> = vec![];

        if value.is_object() {
            let obj = value.as_object().unwrap();
            let arr = JsArray::from_object(obj.clone())?;
            for i in 0..arr.length(context)? {
                let blob_part: BlobPart = arr.get(i, context)?.try_js_into(context)?;
                vec.push(blob_part)
            }
        }

        Ok(Self(vec))
    }
}

#[derive(Default, Clone)]
pub enum Endings {
    #[default]
    Transparent,
    Native,
}

impl TryFromJs for Endings {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let endings = String::try_from_js(value, context)?;
        match endings.as_str() {
            "transparent" => Ok(Endings::Transparent),
            "native" => Ok(Endings::Native),
            _ => Err(JsError::from_native(
                JsNativeError::typ()
                    .with_message("Expected either 'transparent' or 'native'"),
            )),
        }
    }
}

#[derive(Default, Clone)]
pub struct BlobPropertyBag {
    type_: Option<String>,
    endings: Option<Endings>,
}

impl TryFromJs for BlobPropertyBag {
    fn try_from_js(value: &JsValue, context: &mut Context) -> JsResult<Self> {
        let obj = value.as_object().ok_or_else(|| {
            JsError::from_native(JsNativeError::typ().with_message("Expected object"))
        })?;

        let type_: Option<String> = if obj.has_property(js_string!("type"), context)? {
            Some(String::try_from_js(
                &obj.get(js_string!("type"), context)?,
                context,
            )?)
        } else {
            None
        };

        let endings: Option<Endings> =
            if obj.has_property(js_string!("endings"), context)? {
                Some(Endings::try_from_js(
                    &obj.get(js_string!("endings"), context)?,
                    context,
                )?)
            } else {
                None
            };

        Ok(Self { type_, endings })
    }
}

pub struct BlobClass;

impl BlobClass {
    fn size(context: &mut Context) -> Accessor {
        accessor!(
            context,
            Blob,
            "size",
            get:((blob, _context) => Ok(blob.size().into()))
        )
    }

    fn type_(context: &mut Context) -> Accessor {
        accessor!(
            context,
            Blob,
            "type",
            get:((blob, context) => Ok(blob.type_().into_js(context)))
        )
    }

    fn text(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut blob = Blob::try_from_js(this)?;

        Ok(blob.text(context)?.into())
    }

    fn array_buffer(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let mut blob = Blob::try_from_js(this)?;

        Ok(blob.array_buffer(context)?.into())
    }

    fn slice(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let blob = Blob::try_from_js(this)?;
        let start: Option<i64> = args.get_or_undefined(0).try_js_into(context)?;
        let end: Option<i64> = args.get_or_undefined(1).try_js_into(context)?;
        let content_type: Option<String> =
            args.get_or_undefined(2).try_js_into(context)?;
        let blob = blob.slice(start, end, content_type);
        let blob = JsNativeObject::new::<BlobClass>(blob, context)?;

        Ok(blob.to_inner())
    }
}

impl NativeClass for BlobClass {
    type Instance = Blob;

    const NAME: &'static str = "Blob";

    fn data_constructor(
        _target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<Self::Instance> {
        let blob_parts: Option<BlobParts> =
            args.get_or_undefined(0).try_js_into(context)?;
        let options: Option<BlobPropertyBag> =
            args.get_or_undefined(1).try_js_into(context)?;

        Blob::new(blob_parts, options, context)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        let size = Self::size(class.context());
        let type_ = Self::type_(class.context());

        class
            .accessor(js_string!("size"), size, Attribute::all())
            .accessor(js_string!("type"), type_, Attribute::all())
            .method(
                js_string!("text"),
                0,
                NativeFunction::from_fn_ptr(Self::text),
            )
            .method(
                js_string!("arrayBuffer"),
                0,
                NativeFunction::from_fn_ptr(Self::array_buffer),
            )
            .method(
                js_string!("slice"),
                0,
                NativeFunction::from_fn_ptr(Self::slice),
            );

        Ok(())
    }
}

pub struct BlobApi;

impl jstz_core::Api for BlobApi {
    fn init(self, context: &mut Context) {
        register_global_class::<BlobClass>(context)
            .expect("The `Blob` class shouldn't exist yet")
    }
}
