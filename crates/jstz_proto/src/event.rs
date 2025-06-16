#![allow(unused)]
use bincode::{Decode, Encode};
use jstz_core::host::HostRuntime;
use nom::{bytes::complete::tag, InputTake};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::prelude::debug_msg;

use crate::runtime::v2::oracle::OracleRequest;

/// Jstz Events
pub trait Event: PartialEq + Serialize {
    fn tag() -> &'static str;
}

/// Responsible for publishing events to the kernel debug log
#[derive(Default)]
pub struct EventPublisher;

impl EventPublisher {
    /// Jstz events are published as single line in the kernel debug log with the
    /// schema "[JSTZ]<json payload>
    pub(crate) fn publish_event<R, E: Event>(&mut self, rt: &R, event: &E) -> Result<()>
    where
        R: HostRuntime,
    {
        let json = serde_json::to_string(event).map_err(EncodeError::from)?;
        let prefix = E::tag();
        debug_msg!(rt, "[{prefix}]{json}");
        Ok(())
    }
}

pub fn decode_line<'de, E: Event + Deserialize<'de>>(input: &'de str) -> Result<E> {
    let str = parse_line::<E>(input)?;
    Ok(serde_json::from_str(str).map_err(DecodeError::from)?)
}

fn parse_line<E: Event>(input: &str) -> std::result::Result<&str, DecodeError> {
    let prefix = format!("[{}]", E::tag());
    let (input, _) = tag::<&str, &str, NomError>(&prefix)(input)?;
    Ok(input)
}

pub type Result<T> = std::result::Result<T, EventError>;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Error while encoding event: {0}")]
    Encode(#[from] EncodeError),
    #[error("Error while decoding event: {0}")]
    Decode(#[from] DecodeError),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum EncodeError {
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("{0}")]
    Nom(#[from] nom::Err<NomError>),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

const TRUNCATE_LEN: usize = 30;

fn truncate(input: &str, truncate_len: usize) -> String {
    if input.len() > truncate_len {
        let remaining = input.len() - truncate_len;
        let substr = input.take(truncate_len);
        format!("{}[{}..]", substr, remaining)
    } else {
        input.to_string()
    }
}

#[derive(Debug, derive_more::Display)]
pub struct NomError(String);

impl nom::error::ParseError<&str> for NomError {
    fn from_error_kind(input: &str, kind: nom::error::ErrorKind) -> Self {
        let string_or_truncate = truncate(input, TRUNCATE_LEN);
        let message = format!(
            "Nom decode failed: kind '{}' on input '{}'",
            kind.description(),
            string_or_truncate
        );
        NomError(message)
    }

    fn append(input: &str, kind: nom::error::ErrorKind, NomError(child): Self) -> Self {
        let message = format!(
            "Nom decode failed while decoding kind '{}' for '{}'\n\t{}",
            kind.description(),
            truncate(input, 10),
            child
        );
        NomError(message)
    }
}

#[cfg(test)]
mod test {

    use std::{fmt::Display, str::FromStr};

    use http::HeaderMap;
    use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
    use serde_json::json;
    use tezos_smart_rollup_mock::MockHost;
    use url::Url;

    use crate::{
        event::{decode_line, DecodeError, Event, EventError, EventPublisher, NomError},
        runtime::v2::{
            fetch::http::{Body, Request},
            oracle::OracleRequest,
        },
    };

    pub struct Sink(pub Vec<u8>);

    impl Sink {
        #[allow(unused)]
        pub fn lines(&self) -> Vec<String> {
            self.to_string()
                .split("\n")
                .map(ToString::to_string)
                .collect()
        }
    }

    impl Display for Sink {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", String::from_utf8_lossy(self.0.as_ref()))
        }
    }

    #[test]
    fn test_publish_decode_roundtrip() {
        let mut sink = Sink(Vec::new());
        let mut host = MockHost::default();
        host.set_debug_handler(unsafe {
            std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                &mut sink.0,
            )
        });
        let event = OracleRequest {
            id: 1,
            caller: PublicKeyHash::from_base58("tz1XSYefkGnDLgkUPUmda57jk1QD6kqk2VDb")
                .unwrap(),
            gas_limit: 100,
            timeout: 21,
            request: Request {
                method: "POST".into(),
                url: Url::from_str("http://example.com/foo").unwrap(),
                headers: vec![],
                body: Some(Body::Vector(
                    serde_json::to_vec(&json!({ "message": "hello"})).unwrap(),
                )),
            },
        };
        let mut publisher = EventPublisher::default();
        publisher.publish_event(&mut host, &event);
        let head_line = sink.lines().first().unwrap().clone();
        assert_eq!(
            head_line,
            r#"[ORACLE]{"id":1,"caller":"tz1XSYefkGnDLgkUPUmda57jk1QD6kqk2VDb","gas_limit":100,"timeout":21,"request":{"method":[80,79,83,84],"url":"http://example.com/foo","headers":[],"body":{"Vector":[123,34,109,101,115,115,97,103,101,34,58,34,104,101,108,108,111,34,125]}}}"#
        );
        let decoded = decode_line(&head_line).unwrap();
        assert_eq!(event, decoded)
    }

    #[test]
    fn fails_decode_on_invalid_line() {
        let decoded = decode_line::<OracleRequest>("invalid line").unwrap_err();
        assert_eq!(
            decoded.to_string(),
            "Error while decoding event: Parsing Error: NomError(\"Nom decode failed: kind 'Tag' on input 'invalid line'\")"
        );

        let decoded =
            decode_line::<OracleRequest>(r#"[ORACLE]{"message": "boom"}"#).unwrap_err();
        assert_eq!(
            decoded.to_string(),
            "Error while decoding event: missing field `id` at line 1 column 19"
        )
    }
}
