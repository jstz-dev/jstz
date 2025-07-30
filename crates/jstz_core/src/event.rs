use crate::host::HostRuntime;
use nom::{bytes::complete::tag, InputTake};
use serde::{de::DeserializeOwned, Serialize};
use tezos_smart_rollup::prelude::debug_msg;

/// Jstz Events
pub trait Event: PartialEq + Serialize + DeserializeOwned {
    fn tag() -> &'static str;
}

/// Responsible for publishing events to the kernel debug log
#[derive(Debug, Default)]
pub struct EventPublisher;

impl EventPublisher {
    /// Jstz events are published as single line in the kernel debug log with the
    /// schema "[Event::tag()]<json payload>\n"
    pub fn publish_event<R, E: Event>(rt: &R, event: &E) -> Result<()>
    where
        R: HostRuntime,
    {
        let json = serde_json::to_string(event).map_err(EncodeError::from)?;
        let prefix = E::tag();
        debug_msg!(rt, "[{prefix}]{json}\n");
        Ok(())
    }
}

pub fn decode_line<E: Event>(input: &str) -> Result<E> {
    let input = input.trim();
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
        format!("{substr}[{remaining}..]")
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

    use crate::event::{decode_line, Event, EventPublisher, NomError};
    use bincode::{Decode, Encode};
    use jstz_crypto::{hash::Hash, public_key_hash::PublicKeyHash};
    use nom::error::ParseError;
    use serde::{Deserialize, Serialize};
    use std::{fmt::Display, str::FromStr};
    use tezos_smart_rollup_mock::MockHost;
    use url::Url;

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

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
    pub struct MockEvent {
        pub id: u64,
        pub caller: PublicKeyHash,
        pub gas_limit: u64,
        #[bincode(with_serde)]
        pub url: Url,
    }

    impl Event for MockEvent {
        fn tag() -> &'static str {
            "MOCK"
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
        let event = MockEvent {
            id: 1,
            caller: PublicKeyHash::from_base58("tz1XSYefkGnDLgkUPUmda57jk1QD6kqk2VDb")
                .unwrap(),
            gas_limit: 100,
            url: Url::from_str("http://example.com/foo").unwrap(),
        };
        EventPublisher::publish_event(&host, &event).unwrap();
        let head_line = sink.lines().first().unwrap().clone();
        assert_eq!(
            head_line,
            r#"[MOCK]{"id":1,"caller":"tz1XSYefkGnDLgkUPUmda57jk1QD6kqk2VDb","gas_limit":100,"url":"http://example.com/foo"}"#
        );
        let decoded = decode_line::<MockEvent>(&head_line).unwrap();
        assert_eq!(event, decoded)
    }

    #[test]
    fn fails_decode_on_invalid_line() {
        let decoded = decode_line::<MockEvent>("invalid line").unwrap_err();
        assert_eq!(
            decoded.to_string(),
            "Error while decoding event: Parsing Error: NomError(\"Nom decode failed: kind 'Tag' on input 'invalid line'\")"
        );

        let decoded =
            decode_line::<MockEvent>(r#"[MOCK]{"message": "boom"}"#).unwrap_err();
        assert_eq!(
            decoded.to_string(),
            "Error while decoding event: missing field `id` at line 1 column 19"
        )
    }

    #[test]
    fn test_nomerror_append() {
        use nom::error::ErrorKind;
        let child = NomError::from_error_kind("childinput", ErrorKind::Alpha);
        let appended = NomError::append("parentinput", ErrorKind::Alpha, child);
        let msg = appended.to_string();
        assert!(msg.contains("while decoding kind 'Alphabetic' for 'parentinpu[1..]'"));
        assert!(msg.contains("kind 'Alphabetic' on input 'childinput'"));
    }
}
