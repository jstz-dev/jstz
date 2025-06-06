use std::future;

use bytes::Bytes;
use deno_core::{ByteString, JsBuffer};
use deno_fetch_base::BytesStream;
use futures::stream;
use http::{HeaderMap, HeaderName, HeaderValue};
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

impl Into<http::Response<Option<Vec<u8>>>> for Response {
    fn into(self) -> http::Response<Option<Vec<u8>>> {
        // According to JavaScript documentation, `Response.error()` returns a response with status code 0
        // and is mainly used for client side network errors. In regular JS, the fetch promise would be rejected.
        // Within Jstz, the smart function can only return this if it explicitly called `Response.error()`, which
        // means the intent is closer to a 400 Bad Request.
        let status = if self.status == 0 { 400 } else { self.status };
        let mut builder = http::Response::builder().status(status);

        let headers =
            HeaderMap::from_iter(self.headers.into_iter().map(|(key, value)| {
                (
                    HeaderName::from_bytes(&key)
                        .expect("Expected valid http header key from a valid response"),
                    HeaderValue::from_bytes(&value)
                        .expect("Expected valid http header value from a valid response"),
                )
            }));
        *builder.headers_mut().unwrap() = headers;

        let body = if self.body.is_empty() {
            None
        } else {
            Some(self.body.to_vec())
        };

        builder
            .body(body)
            .expect("Expected valid http response from a valid response")
    }
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

/// Converts http::HeaderMap instances to the format accepted by the v2 runtime.
#[allow(unused)]
pub fn convert_header_map(headers: HeaderMap) -> Vec<(ByteString, ByteString)> {
    let mut res = Vec::new();
    let mut curr_key = None;
    // According to the documentation, for each yielded item that has `None` provided for the
    // HeaderName, the associated header name is the same as that of the previously yielded item.
    // The first yielded item will have HeaderName set.
    // Therefore the assert should never fail.
    for (key, value) in headers.into_iter() {
        if key.is_some() {
            curr_key = key;
        }
        match curr_key {
            Some(ref k) => {
                res.push((k.as_str().as_bytes().into(), value.as_bytes().into()));
            }
            None => panic!("current header key should not be none"),
        }
    }
    res
}

#[cfg(test)]
mod test {
    use futures::StreamExt;
    use http::{HeaderMap, HeaderName, HeaderValue};

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

    #[test]
    fn response_to_http_response() {
        let response = super::Response {
            status: 403,
            status_text: "something".to_string(),
            headers: vec![
                ("k1".as_bytes().into(), "v1".as_bytes().into()),
                ("k2".as_bytes().into(), "v3".as_bytes().into()),
                ("k2".as_bytes().into(), "v2".as_bytes().into()),
                ("k1".as_bytes().into(), "v4".as_bytes().into()),
            ],
            body: super::Body::Vector(vec![1, 2, 3]),
        };
        let http_response: http::Response<Option<Vec<u8>>> = response.into();
        let (parts, body) = http_response.into_parts();
        assert_eq!(body, Some(vec![1, 2, 3]));
        assert_eq!(parts.status, http::StatusCode::FORBIDDEN);
        let mut expected_headers = HeaderMap::new();
        // Ordering doesn't matter; HeaderMap handles it during comparison.
        // The key is that our implementation also allows duplicated keys.
        expected_headers.append(
            HeaderName::from_static("k1"),
            HeaderValue::from_static("v1"),
        );
        expected_headers.append(
            HeaderName::from_static("k2"),
            HeaderValue::from_static("v3"),
        );
        expected_headers.append(
            HeaderName::from_static("k2"),
            HeaderValue::from_static("v2"),
        );
        expected_headers.append(
            HeaderName::from_static("k1"),
            HeaderValue::from_static("v4"),
        );
        assert_eq!(parts.headers.len(), 4);
        assert_eq!(parts.headers, expected_headers);
    }

    #[test]
    fn response_to_http_response_empty_body() {
        let response = super::Response {
            status: 401,
            status_text: "something".to_string(),
            headers: vec![],
            body: super::Body::Vector(vec![]),
        };
        let http_response: http::Response<Option<Vec<u8>>> = response.into();
        let body = http_response.into_body();
        assert_eq!(body, None);
    }

    #[test]
    fn convert_header_map() {
        let mut m = HeaderMap::new();
        m.append("k1", HeaderValue::from_str("v1").unwrap());
        m.append("k2", HeaderValue::from_str("v2").unwrap());
        m.append("k2", HeaderValue::from_str("v3").unwrap());
        m.append("k3", HeaderValue::from_str("v4").unwrap());
        m.append("k2", HeaderValue::from_str("v5").unwrap());

        let res = super::convert_header_map(m);
        let expected = [
            ("k1", "v1"),
            ("k2", "v2"),
            ("k2", "v3"),
            ("k2", "v5"),
            ("k3", "v4"),
        ]
        .map(|(k, v)| (k.as_bytes().into(), v.as_bytes().into()));
        assert_eq!(expected, *res);
    }
}
