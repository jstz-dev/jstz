use std::future;

use bytes::Bytes;
use deno_core::{ByteString, JsBuffer};
use deno_fetch_base::BytesStream;
use futures::stream;
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

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Vector(v) => v.is_empty(),
            Self::Buffer(b) => b.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Vector(v) => v.len(),
            Self::Buffer(b) => b.len(),
        }
    }
}

impl From<Body> for BytesStream {
    fn from(body: Body) -> Self {
        if body.is_empty() {
            return Box::pin(stream::empty());
        }
        Box::pin(stream::once(future::ready(Ok(body.into()))))
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

impl From<Body> for Bytes {
    fn from(body: Body) -> Self {
        match body {
            Body::Vector(items) => Bytes::from(items),
            Body::Buffer(js_buffer) => Bytes::from(js_buffer.to_vec()),
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

#[cfg(test)]
mod test {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn test_response_body() {
        let inner = vec![1, 2, 3, 4, 5];
        let response_body = Body::Vector(inner.clone());
        assert!(response_body.len() > 0);
        assert_eq!(response_body.len(), inner.len());

        // Test converting to BytesStream
        let stream: BytesStream = response_body.into();
        let mut stream = stream;
        let result = stream.next().await;
        assert!(result.is_some());
        let bytes = result.unwrap().unwrap();
        assert_eq!(bytes, Bytes::from(inner));

        // Test empty stream
        let empty_response_body = Body::Vector(vec![]);
        assert_eq!(empty_response_body.len(), 0);
        let stream: BytesStream = empty_response_body.into();
        let mut stream = stream;
        assert!(stream.next().await.is_none());
    }
}
