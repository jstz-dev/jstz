#![allow(unused)]
use bincode::{Decode, Encode};
use jstz_core::host::HostRuntime;
use nom::{bytes::complete::tag, InputTake};
use serde::{Deserialize, Serialize};
use tezos_smart_rollup::prelude::debug_msg;

use crate::runtime::v2::oracle::OracleRequest;

pub const JSTZ_PREFIX: &str = "[JSTZ]";

/// Jstz Events
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    OracleRequest(OracleRequest),
}

/// Responsible for publishing events to the kernel debug log
#[derive(Default)]
pub struct EventPublisher;

impl EventPublisher {
    /// Jstz events are published as single line in the kernel debug log with the
    /// schema "[JSTZ]<json payload>
    pub(crate) fn publish_event<R>(&mut self, rt: &R, event: &Event) -> Result<()>
    where
        R: HostRuntime,
    {
        let json = serde_json::to_string(event).map_err(EncodeError::from)?;
        debug_msg!(rt, "{JSTZ_PREFIX}{json}");
        Ok(())
    }
}

pub fn decode_line(input: &str) -> Result<Event> {
    let str = parse_line(input)?;
    Ok(serde_json::from_str(str).map_err(DecodeError::from)?)
}

fn parse_line(input: &str) -> std::result::Result<&str, DecodeError> {
    let (input, _) = tag::<&str, &str, NomError>(JSTZ_PREFIX)(input)?;
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
        event::{decode_line, Event, EventPublisher},
        runtime::v2::{
            fetch::{Body, Request},
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
        let event = Event::OracleRequest(OracleRequest {
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
        });
        let mut publisher = EventPublisher::default();
        publisher.publish_event(&mut host, &event);
        let head_line = sink.lines().first().unwrap().clone();
        let decoded = decode_line(&head_line).unwrap();
        assert_eq!(event, decoded)
    }
}
