use deno_core::{ByteString, JsBuffer};
use url::Url;

use crate::context::account::Address;

use super::error::*;

use jstz_runtime::sys::ToV8;

use deno_core::{serde_v8, v8, ToJsBuffer};

/// Response returned from a fetch or Smart Function run
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(ByteString, ByteString)>,
    pub body: Body,
}

#[derive(Debug)]
pub enum Body {
    Vector(Vec<u8>),
    Buffer(JsBuffer),
}

impl Body {
    #[allow(unused)]
    pub fn to_vec(self) -> Vec<u8> {
        self.into()
    }

    pub fn zero_capacity() -> Self {
        Self::Vector(Vec::with_capacity(0))
    }
}

impl From<Body> for Vec<u8> {
    fn from(body: Body) -> Self {
        match body {
            Body::Vector(items) => items,
            Body::Buffer(js_buffer) => js_buffer.to_vec(),
        }
    }
}

impl<'s> ToV8<'s> for Body {
    fn to_v8(
        self,
        scope: &mut v8::HandleScope<'s>,
    ) -> jstz_runtime::error::Result<v8::Local<'s, v8::Value>> {
        match self {
            Body::Vector(items) => {
                let to_buffer = ToJsBuffer::from(items);
                let value = serde_v8::to_v8(scope, to_buffer)?;
                Ok(value)
            }
            Body::Buffer(js_buffer) => js_buffer.to_v8(scope),
        }
    }
}

pub enum SupportedScheme {
    Jstz,
}

impl TryFrom<&Url> for SupportedScheme {
    type Error = FetchError;

    fn try_from(value: &Url) -> Result<Self> {
        match value.scheme() {
            "jstz" => Ok(Self::Jstz),
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_string())),
        }
    }
}

impl TryFrom<&Url> for Address {
    type Error = FetchError;

    fn try_from(url: &Url) -> Result<Self> {
        let raw_address = url.host().ok_or(url::ParseError::EmptyHost)?;
        Address::from_base58(raw_address.to_string().as_str())
            .map_err(|err| FetchError::JstzError(err.to_string()))
    }
}
