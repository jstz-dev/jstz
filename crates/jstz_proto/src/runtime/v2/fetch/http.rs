use deno_core::{ByteString, JsBuffer};
use url::Url;

use crate::context::account::Address;

use super::error::*;

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
