use deno_core::{ByteString, JsBuffer};
use http::{HeaderMap, HeaderName, HeaderValue};
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

impl Response {
    pub fn to_http_response(self) -> http::Response<Option<Vec<u8>>> {
        let mut builder = http::Response::builder().status(self.status);

        let headers =
            HeaderMap::from_iter(self.headers.into_iter().map(|(key, value)| {
                let key_vec: Vec<u8> = key.into();
                let value_vec: Vec<u8> = value.into();
                (
                    HeaderName::from_bytes(&key_vec)
                        .expect("Expected valid http header key from a valid response"),
                    HeaderValue::from_bytes(&value_vec)
                        .expect("Expected valid http header value from a valid response"),
                )
            }));

        *builder.headers_mut().unwrap() = headers;

        let body_vec = self.body.to_vec();
        let body = if body_vec.is_empty() {
            None
        } else {
            Some(body_vec)
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

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderName, HeaderValue};

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
        let http_response = response.to_http_response();
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
        let http_response = response.to_http_response();
        let body = http_response.into_body();
        assert_eq!(body, None);
    }
}
