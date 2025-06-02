use deno_core::{ByteString, JsBuffer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use crate::context::account::Address;

use super::error::*;

/// Response returned from fetch or [`crate::operation::RunFunction`]
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(ByteString, ByteString)>,
    pub body: Body,
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
        let json = serde_json::to_value(request).unwrap();
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
    }
}
