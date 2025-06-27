use std::future;

use bytes::Bytes;
use deno_core::{ByteString, JsBuffer};
use deno_fetch_base::BytesStream;
use futures::stream;
use http::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use crate::context::account::Address;

use super::error::*;

use jstz_runtime::sys::ToV8;

use deno_core::{serde_v8, v8, ToJsBuffer};

use crate::executor::smart_function::JSTZ_HOST;

/// Response returned from fetch or [`crate::operation::RunFunction`]
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    #[serde(with = "serde_bytestring")]
    pub method: ByteString,
    pub url: Url,
    #[serde(with = "serde_vec_tuple_bytestring")]
    pub headers: Vec<(ByteString, ByteString)>,
    pub body: Option<Body>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Body {
    Vector(Vec<u8>),
    Buffer(JsBuffer),
}

impl PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Vector(l0), Self::Vector(r0)) => l0 == r0,
            (Self::Buffer(l0), Self::Buffer(r0)) => l0.to_vec() == r0.to_vec(),
            _ => false,
        }
    }
}

impl Eq for Body {}

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

impl From<String> for Body {
    fn from(s: String) -> Self {
        Body::Vector(s.as_bytes().to_vec())
    }
}

impl From<&str> for Body {
    fn from(s: &str) -> Self {
        Body::Vector(s.as_bytes().to_vec())
    }
}

impl From<&[u8]> for Body {
    fn from(bytes: &[u8]) -> Self {
        Body::Vector(bytes.to_vec())
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

pub enum HostName {
    Address(Address),
    JstzHost,
}

impl TryFrom<&Url> for HostName {
    type Error = FetchError;

    fn try_from(url: &Url) -> Result<Self> {
        let to = Address::try_from(url);
        match to {
            Ok(to) => Ok(Self::Address(to)),
            Err(e) => match url.domain() {
                Some(JSTZ_HOST) => Ok(Self::JstzHost),
                _ => Err(e),
            },
        }
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

pub mod serde_bytestring {
    use serde_bytes::{ByteBuf, Bytes};

    use super::*;

    pub fn serialize<S>(
        bytes: &ByteString,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Bytes::new(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<ByteString, D::Error>
    where
        D: Deserializer<'de>,
    {
        let buf = ByteBuf::deserialize(deserializer)?;
        Ok(ByteString::from(buf.as_slice()))
    }
}

pub mod serde_vec_tuple_bytestring {
    use super::*;
    use serde::de::{SeqAccess, Visitor};
    use serde::ser::SerializeSeq;
    use serde::{Deserializer, Serializer};
    use serde_bytes::{ByteBuf, Bytes};
    use std::fmt;

    pub fn serialize<S>(
        vec: &Vec<(ByteString, ByteString)>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(vec.len()))?;
        for (k, v) in vec {
            seq.serialize_element(&(Bytes::new(k), Bytes::new(v)))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Vec<(ByteString, ByteString)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TupleBytesVisitor;

        impl<'de> Visitor<'de> for TupleBytesVisitor {
            type Value = Vec<(ByteString, ByteString)>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of byte tuples")
            }

            fn visit_seq<A>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some((k, v)) = seq.next_element::<(ByteBuf, ByteBuf)>()? {
                    values.push((
                        ByteString::from(k.as_slice()),
                        ByteString::from(v.as_slice()),
                    ));
                }
                Ok(values)
            }
        }

        deserializer.deserialize_seq(TupleBytesVisitor)
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr as _;

    use bytes::Bytes;
    use deno_fetch_base::BytesStream;
    use futures::StreamExt;
    use http::{HeaderMap, HeaderName, HeaderValue};
    use serde_json::json;
    use url::Url;

    use super::{Body, Request};

    #[test]
    fn request_json_roundtrip() {
        let request = Request {
            method: "POST".into(),
            url: Url::from_str("http://example.com/foo").unwrap(),
            headers: vec![],
            body: Some(Body::Vector(
                serde_json::to_vec(&json!({ "message": "hello"})).unwrap(),
            )),
        };
        let json = serde_json::to_value(request.clone()).unwrap();
        assert_eq!(
            json!({
                "method":[80,79,83,84],
                "url":"http://example.com/foo",
                "headers":[],
                "body":{
                    "Vector":[123,34,109,101,115,115,97,103,101,34,58,34,104,101,108,108,111,34,125]
                }
            }),
            json
        );

        let json = json.to_string();

        let de: Request = serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(request, de);
    }

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
